//! Error classification: exit code first, stderr substring second.
//! Phrasings vary per command and per version, so matching is defensive and the
//! raw stderr is always preserved for the message log.

/// A classified CLI failure. `raw` on every variant keeps the verbatim stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// XPC connection error — the container system service is not running.
    ServiceDown {
        raw: String,
    },
    /// The entity vanished between poll and action; status-bar notice only.
    NotFound {
        raw: String,
    },
    /// e.g. deleting a running container or an in-use volume.
    InUse {
        raw: String,
    },
    /// Exit 64 — bushel built a bad command line; a bushel bug, not a user error.
    Usage {
        raw: String,
    },
    /// stdout was not the JSON shape we expected.
    ParseFailure {
        raw: String,
    },
    /// A read exceeded its deadline (mutating actions have none).
    Timeout,
    Other {
        raw: String,
    },
}

impl CliError {
    /// Classify a non-zero exit. `stderr` is the full captured stream.
    pub fn classify(code: i32, stderr: &str) -> Self {
        let raw = stderr.trim().to_string();
        if code == 64 {
            return CliError::Usage { raw };
        }
        let lower = raw.to_lowercase();
        if lower.contains("xpc connection error")
            || lower.contains("ensure container system service")
            || lower.contains("apiserver is not running")
        {
            CliError::ServiceDown { raw }
        } else if lower.contains("not found") {
            CliError::NotFound { raw }
        } else if lower.contains("invalidstate")
            || lower.contains("is running and can not be deleted")
            || lower.contains("in use")
        {
            CliError::InUse { raw }
        } else {
            CliError::Other { raw }
        }
    }

    /// Full stderr for the message log.
    pub fn raw(&self) -> &str {
        match self {
            CliError::ServiceDown { raw }
            | CliError::NotFound { raw }
            | CliError::InUse { raw }
            | CliError::Usage { raw }
            | CliError::ParseFailure { raw }
            | CliError::Other { raw } => raw,
            CliError::Timeout => "timed out",
        }
    }

    /// One-line gist for the bottom bar.
    pub fn gist(&self) -> String {
        match self {
            CliError::ServiceDown { .. } => "container system service is not running".into(),
            CliError::NotFound { .. } => "not found (already gone?)".into(),
            CliError::Usage { .. } => "bushel bug: invalid command line (see message log)".into(),
            CliError::ParseFailure { .. } => "unexpected CLI output (see message log)".into(),
            CliError::Timeout => "timed out".into(),
            CliError::InUse { .. } | CliError::Other { .. } => self
                .raw()
                .lines()
                .next()
                .unwrap_or("command failed")
                .trim_start_matches("Error: ")
                .to_string(),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.gist())
    }
}

impl std::error::Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("fixtures/1.2.0/stderr/{name}")).unwrap()
    }

    #[test]
    fn xpc_error_classifies_as_service_down() {
        let e = CliError::classify(1, &fixture("service_down_ls.txt"));
        assert!(matches!(e, CliError::ServiceDown { .. }), "{e:?}");
    }

    #[test]
    fn all_three_not_found_phrasings_classify_as_not_found() {
        for f in [
            "not_found_inspect.txt",
            "not_found_start.txt",
            "not_found_stop.txt",
        ] {
            let e = CliError::classify(1, &fixture(f));
            assert!(matches!(e, CliError::NotFound { .. }), "{f}: {e:?}");
        }
    }

    #[test]
    fn delete_running_classifies_as_in_use() {
        let e = CliError::classify(1, &fixture("delete_running.txt"));
        assert!(matches!(e, CliError::InUse { .. }), "{e:?}");
    }

    #[test]
    fn exit_64_classifies_as_usage_regardless_of_stderr() {
        let e = CliError::classify(64, &fixture("usage.txt"));
        assert!(matches!(e, CliError::Usage { .. }), "{e:?}");
    }

    #[test]
    fn unrecognized_stderr_falls_through_to_other_with_raw_preserved() {
        let e = CliError::classify(1, "Error: something novel happened");
        assert!(matches!(e, CliError::Other { .. }));
        assert_eq!(e.raw(), "Error: something novel happened");
        assert_eq!(e.gist(), "something novel happened");
    }

    #[test]
    fn raw_stderr_survives_classification_verbatim_modulo_trim() {
        let raw = fixture("service_down_ls.txt");
        let e = CliError::classify(1, &raw);
        assert_eq!(e.raw(), raw.trim());
    }
}
