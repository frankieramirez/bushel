//! Startup version check. The CLI guarantees output stability only within patch
//! versions; this range is kept next to the fixtures it was validated against.

/// Versions bushel's fixtures were captured on (see `fixtures/<version>/`).
/// Minor range [MIN, MAX] inclusive; any 1.2.x is considered tested.
pub const TESTED_MIN: (u32, u32) = (1, 2);
pub const TESTED_MAX: (u32, u32) = (1, 2);

/// Human-readable range for the mismatch banner.
pub fn tested_range() -> String {
    if TESTED_MIN == TESTED_MAX {
        format!("{}.{}.x", TESTED_MIN.0, TESTED_MIN.1)
    } else {
        format!(
            "{}.{}.x–{}.{}.x",
            TESTED_MIN.0, TESTED_MIN.1, TESTED_MAX.0, TESTED_MAX.1
        )
    }
}

/// Parse `container CLI version 1.2.0 (build: release, commit: 6e65319)` → (1, 2, 0).
pub fn parse(version_line: &str) -> Option<(u32, u32, u32)> {
    let tail = version_line.split("version").nth(1)?.trim();
    let mut parts = tail.split_whitespace().next()?.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

/// Is this version inside the tested range? Unparseable versions are untested.
pub fn is_tested(version_line: &str) -> bool {
    match parse(version_line) {
        Some((maj, min, _)) => (maj, min) >= TESTED_MIN && (maj, min) <= TESTED_MAX,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_real_version_line() {
        assert_eq!(
            parse("container CLI version 1.2.0 (build: release, commit: 6e65319)"),
            Some((1, 2, 0))
        );
    }

    #[test]
    fn any_patch_of_a_tested_minor_is_tested() {
        assert!(is_tested("container CLI version 1.2.0"));
        assert!(is_tested("container CLI version 1.2.9"));
    }

    #[test]
    fn newer_minor_and_major_are_untested() {
        assert!(!is_tested("container CLI version 1.3.0"));
        assert!(!is_tested("container CLI version 2.0.0"));
    }

    #[test]
    fn older_minor_is_untested() {
        assert!(!is_tested("container CLI version 1.1.0"));
    }

    #[test]
    fn garbage_is_untested_not_a_panic() {
        assert!(!is_tested("who knows"));
        assert!(!is_tested(""));
    }

    #[test]
    fn banner_names_the_range() {
        assert_eq!(tested_range(), "1.2.x");
    }
}
