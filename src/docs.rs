//! `docs.json` — bushel's keymap and config reference as data, for the website.
//!
//! bushel.sh builds on Cloudflare's Linux builders and cannot run bushel (macOS
//! 26, Apple silicon), so the docs pages cannot shell out for their reference
//! tables. Instead the release publishes this file as an asset and the site
//! fetches it at build time. Every value here is read from the same source the
//! running program reads — the cheatsheet in [`crate::ui::help`], the fields of
//! [`Config`], the flags of [`Args`] — so the site cannot drift from the binary
//! the way the README's hand-copied keymap did.

use clap::CommandFactory as _;
use serde::Serialize;

use crate::cli::Args;
use crate::config::Config;
use crate::ui::help::HELP;

/// Bumped when the shape below changes in a way the site must react to.
pub const SCHEMA_VERSION: u32 = 1;

/// The whole file.
#[derive(Debug, Serialize)]
pub struct Docs {
    pub schema_version: u32,
    /// The bushel release this was emitted from — also what the site's version
    /// chip renders, so it can no longer go stale.
    pub version: &'static str,
    pub keymap: Vec<KeyGroup>,
    pub config: ConfigDocs,
}

/// One `## heading` of the cheatsheet and the bindings under it.
#[derive(Debug, Serialize)]
pub struct KeyGroup {
    pub group: String,
    pub bindings: Vec<Binding>,
}

#[derive(Debug, Serialize)]
pub struct Binding {
    pub keys: &'static str,
    pub desc: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ConfigDocs {
    /// Where the file goes, in the tilde form the docs quote.
    pub path: &'static str,
    pub options: Vec<ConfigOption>,
}

#[derive(Debug, Serialize)]
pub struct ConfigOption {
    /// The TOML key, e.g. `no_splash`.
    pub key: String,
    /// The flag that sets the same thing, e.g. `--no-splash`.
    pub flag: String,
    /// The default, as a JSON value taken from `Config::default()`.
    pub default: serde_json::Value,
    /// The flag's help text — the flag and the key share one description.
    pub desc: String,
}

/// The two halves of the config reference are derived from different places —
/// keys and defaults from [`Config`], flags and descriptions from [`Args`] — so
/// they can fall out of step. That is a bug in bushel, not in the caller, and
/// it fails the build rather than shipping a half-documented option.
#[derive(Debug, PartialEq, Eq)]
pub enum DocsError {
    /// A `--flag` with no `Config` field of the same name.
    FlagWithoutConfigField(String),
    /// A `Config` field no flag can set.
    ConfigFieldWithoutFlag(String),
    /// A flag clap gave us no help text for.
    FlagWithoutHelp(String),
}

impl std::fmt::Display for DocsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FlagWithoutConfigField(flag) => {
                write!(f, "--{flag} has no matching field on Config")
            }
            Self::ConfigFieldWithoutFlag(key) => {
                write!(f, "Config field `{key}` has no matching --flag")
            }
            Self::FlagWithoutHelp(flag) => write!(f, "--{flag} has no help text to document"),
        }
    }
}

impl std::error::Error for DocsError {}

/// The cheatsheet, regrouped: its headings become groups, its bindings the rows
/// under them. Bindings before the first heading would be dropped, so `HELP`
/// opens with one and a test holds it to that.
pub fn keymap() -> Vec<KeyGroup> {
    let mut groups: Vec<KeyGroup> = Vec::new();
    for row in HELP {
        if row.keys.is_empty() {
            groups.push(KeyGroup {
                group: row.desc.trim().to_string(),
                bindings: Vec::new(),
            });
        } else if let Some(current) = groups.last_mut() {
            current.bindings.push(Binding {
                keys: row.keys,
                desc: row.desc,
            });
        }
    }
    groups
}

/// Walks the CLI in declaration order, pairing each flag with the config field
/// of the same name and the default that field actually has.
pub fn config() -> Result<ConfigDocs, DocsError> {
    // Serializing the real `Default` impl is what makes the defaults honest: a
    // field whose default changes changes this file without anyone editing it.
    let serde_json::Value::Object(mut defaults) = serde_json::to_value(Config::default())
        .expect("Config is a flat struct of bools and always serializes")
    else {
        unreachable!("Config serializes as a JSON object");
    };

    let cmd = Args::command();
    let mut options = Vec::new();
    for arg in cmd.get_arguments() {
        // clap adds these itself; they set nothing in the config file.
        if matches!(arg.get_id().as_str(), "help" | "version") {
            continue;
        }
        let Some(flag) = arg.get_long() else { continue };
        let key = flag.replace('-', "_");
        let default = defaults
            .remove(&key)
            .ok_or_else(|| DocsError::FlagWithoutConfigField(flag.to_string()))?;
        let desc = arg
            .get_help()
            .ok_or_else(|| DocsError::FlagWithoutHelp(flag.to_string()))?
            .to_string();
        options.push(ConfigOption {
            key,
            flag: format!("--{flag}"),
            default,
            desc,
        });
    }

    // Anything left is a config key the CLI cannot set — undocumentable here.
    if let Some(orphan) = defaults.keys().next() {
        return Err(DocsError::ConfigFieldWithoutFlag(orphan.clone()));
    }

    Ok(ConfigDocs {
        path: Config::DOC_PATH,
        options,
    })
}

pub fn build() -> Result<Docs, DocsError> {
    Ok(Docs {
        schema_version: SCHEMA_VERSION,
        version: env!("CARGO_PKG_VERSION"),
        keymap: keymap(),
        config: config()?,
    })
}

/// Pretty-printed, with the trailing newline a file wants.
pub fn to_json() -> Result<String, DocsError> {
    let mut s =
        serde_json::to_string_pretty(&build()?).expect("Docs is plain data and always serializes");
    s.push('\n');
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::draw::help_lines;
    use crate::ui::theme::Theme;

    /// The rendered cheatsheet as plain text, one string per line, laid out
    /// wide enough that nothing wraps into the key column.
    fn rendered_cheatsheet() -> Vec<String> {
        help_lines(&Theme::detect(false), 200)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn the_keymap_matches_what_the_cheatsheet_renders() {
        let rendered = rendered_cheatsheet();
        let groups = keymap();
        assert!(!groups.is_empty(), "the cheatsheet has groups");

        // Every group heading is drawn, and every binding under it is drawn as
        // one row carrying both its keys and its description.
        for group in &groups {
            assert!(
                rendered.iter().any(|l| l.trim() == group.group),
                "group heading `{}` is not rendered: {rendered:#?}",
                group.group
            );
            for b in &group.bindings {
                assert!(
                    rendered
                        .iter()
                        .any(|l| l.contains(b.keys) && l.contains(b.desc)),
                    "binding `{}` / `{}` is not rendered: {rendered:#?}",
                    b.keys,
                    b.desc
                );
            }
        }

        // …and nothing is drawn that the keymap does not carry: at this width
        // every row is either a heading or exactly one binding.
        let documented = groups.len() + groups.iter().map(|g| g.bindings.len()).sum::<usize>();
        assert_eq!(
            rendered.len(),
            documented,
            "the cheatsheet renders rows the keymap does not carry"
        );
    }

    #[test]
    fn no_binding_is_dropped_before_the_first_group() {
        assert!(
            HELP.first().is_some_and(|r| r.keys.is_empty()),
            "the cheatsheet must open with a group heading, or `keymap()` drops the rows above it"
        );
    }

    #[test]
    fn the_bindings_the_readme_missed_are_in_the_output() {
        let json = to_json().expect("docs.json builds");
        let keys: Vec<&'static str> = keymap()
            .iter()
            .flat_map(|g| g.bindings.iter().map(|b| b.keys))
            .collect();
        // The four the README's hand-copied keymap had drifted behind.
        for missed in ["f", "b", "u", "pgup/pgdn"] {
            assert!(
                keys.contains(&missed),
                "`{missed}` is missing from the emitted keymap"
            );
            assert!(
                json.contains(missed),
                "`{missed}` is missing from docs.json"
            );
        }
    }

    #[test]
    fn every_config_field_has_a_flag_and_a_default() {
        let docs = config().expect("every Config field pairs with a flag");
        assert_eq!(docs.path, "~/.config/bushel/config.toml");
        let pairs: Vec<(&str, &str)> = docs
            .options
            .iter()
            .map(|o| (o.key.as_str(), o.flag.as_str()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("no_splash", "--no-splash"),
                ("reduced_motion", "--reduced-motion"),
                ("ascii", "--ascii"),
            ]
        );
        for o in &docs.options {
            assert_eq!(o.default, serde_json::json!(false), "{} default", o.key);
            assert!(!o.desc.is_empty(), "{} has help text", o.key);
        }
    }

    #[test]
    fn the_file_is_json_and_carries_the_running_version() {
        let json = to_json().expect("docs.json builds");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["schema_version"], SCHEMA_VERSION);
        assert_eq!(parsed["version"], env!("CARGO_PKG_VERSION"));
        assert!(json.ends_with('\n'), "files end with a newline");
    }
}
