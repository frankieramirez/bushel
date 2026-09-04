use serde::{Deserialize, Serialize};

/// How the body spends the terminal.
///
/// `Rail` is ADR 0002's unified rail, tightened: four borderless sections in one
/// column beside (or above) the detail pane. `Table` makes the resource type a
/// mode instead: one full-width table on top, one full-width detail below.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum LayoutMode {
    #[default]
    Rail,
    Table,
}

impl LayoutMode {
    pub fn title(self) -> &'static str {
        match self {
            Self::Rail => "rail",
            Self::Table => "table",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Rail => "all four panes beside the detail pane",
            Self::Table => "one wide table above one wide detail",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Rail => Self::Table,
            Self::Table => Self::Rail,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub no_splash: bool,
    pub reduced_motion: bool,
    pub ascii: bool,
    pub layout: LayoutMode,
}

impl Config {
    pub const DOC_PATH: &'static str = "~/.config/bushel/config.toml";

    /// Overrides the config directory. Lets a test — or a second profile —
    /// point bushel somewhere other than the real dotfile.
    pub const DIR_ENV: &'static str = "BUSHEL_CONFIG_DIR";

    pub fn dir() -> Option<std::path::PathBuf> {
        if let Some(dir) = std::env::var_os(Self::DIR_ENV) {
            return Some(std::path::PathBuf::from(dir));
        }
        if let Some(home) = dirs::home_dir() {
            let xdg = home.join(".config").join("bushel");
            return Some(xdg);
        }
        dirs::config_dir().map(|d| d.join("bushel"))
    }

    pub fn path() -> Option<std::path::PathBuf> {
        Self::dir().map(|d| d.join("config.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match toml::from_str(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("bushel: ignoring invalid config at {}: {e}", path.display());
                Self::default()
            }
        }
    }

    /// Write the whole config back to `~/.config/bushel/config.toml`.
    ///
    /// The settings panel is the only caller: what the panel shows is what the
    /// file gets, so a round-trip through `load()` returns the same struct.
    pub fn save(&self) -> std::io::Result<std::path::PathBuf> {
        let path = Self::path().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no home directory to write the config into",
            )
        })?;
        let body = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, body)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_layout_is_the_rail() {
        assert_eq!(Config::default().layout, LayoutMode::Rail);
    }

    #[test]
    fn layout_round_trips_through_toml_in_kebab_case() {
        let cfg = Config {
            layout: LayoutMode::Table,
            ascii: true,
            ..Config::default()
        };
        let text = toml::to_string_pretty(&cfg).expect("config serializes");
        assert!(text.contains("layout = \"table\""), "{text}");
        assert_eq!(toml::from_str::<Config>(&text).expect("round trip"), cfg);
    }

    #[test]
    fn an_unknown_layout_is_a_parse_error_not_a_silent_rail() {
        assert!(toml::from_str::<Config>("layout = \"grid\"").is_err());
    }

    #[test]
    fn a_config_without_layout_still_loads() {
        let cfg: Config = toml::from_str("ascii = true").expect("partial config loads");
        assert!(cfg.ascii);
        assert_eq!(cfg.layout, LayoutMode::Rail);
    }

    #[test]
    fn an_env_override_moves_the_whole_config_directory() {
        // SAFETY: single-threaded within this test, and the value is restored.
        unsafe { std::env::set_var(Config::DIR_ENV, "/tmp/bushel-config-test") };
        assert_eq!(
            Config::path(),
            Some(std::path::PathBuf::from(
                "/tmp/bushel-config-test/config.toml"
            ))
        );
        unsafe { std::env::remove_var(Config::DIR_ENV) };
    }

    #[test]
    fn the_two_modes_cycle_into_each_other() {
        assert_eq!(LayoutMode::Rail.next(), LayoutMode::Table);
        assert_eq!(LayoutMode::Table.next(), LayoutMode::Rail);
    }
}
