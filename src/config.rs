use serde::{Deserialize, Serialize};

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

    /// Where `save` writes, phrased for a reader: the tilde form unless
    /// `BUSHEL_CONFIG_DIR` has moved it somewhere else.
    pub fn display_path() -> String {
        match (std::env::var_os(Self::DIR_ENV), Self::path()) {
            (Some(_), Some(path)) => path.display().to_string(),
            _ => Self::DOC_PATH.to_string(),
        }
    }

    pub fn load() -> PersistedConfig {
        match Self::path() {
            Some(path) => Self::load_from(path),
            None => PersistedConfig {
                error: Some("no home directory to read or write the config".into()),
                ..PersistedConfig::default()
            },
        }
    }

    /// Load an explicit path, including diagnostics, without touching process
    /// environment variables. Missing files start with an empty document.
    pub fn load_from(path: std::path::PathBuf) -> PersistedConfig {
        let mut loaded = PersistedConfig {
            path: Some(path.clone()),
            ..PersistedConfig::default()
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                loaded.source = Some(text.clone());
                match toml::from_str::<Config>(&text) {
                    Ok(config) => {
                        // Effective settings remain valid even when the lossless
                        // editor supports less TOML than the existing loader.
                        loaded.config = config;
                        match text.parse::<toml_edit::DocumentMut>() {
                            Ok(document) => loaded.document = document,
                            Err(e) => {
                                loaded.error = Some(format!(
                                    "cannot edit config losslessly at {}: {e}; existing settings loaded, settings writes disabled",
                                    path.display()
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        loaded.error = Some(format!("invalid config at {}: {e}", path.display()))
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                loaded.error = Some(format!("could not read config at {}: {e}", path.display()))
            }
        }
        loaded
    }
}

/// The original file document and diagnostics, kept apart from CLI overrides.
#[derive(Debug, Clone, Default)]
pub struct PersistedConfig {
    path: Option<std::path::PathBuf>,
    source: Option<String>,
    document: toml_edit::DocumentMut,
    config: Config,
    error: Option<String>,
}

impl PersistedConfig {
    pub fn effective(&self) -> Config {
        self.config
    }

    pub fn load_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Persist only one known setting, preserving all other document content.
    /// Invalid or externally changed files are left untouched. Symbolic links
    /// are refused because atomic rename would replace the link itself; both
    /// the link and its target retain their original contents.
    pub fn save_setting(
        &mut self,
        key: &str,
        from: &Config,
    ) -> std::io::Result<std::path::PathBuf> {
        use std::io::Write as _;
        if let Some(error) = &self.error {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.clone(),
            ));
        }
        let path = self.path.as_ref().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no config path to write into")
        })?;
        let mut value = match key {
            "layout" => toml_edit::Value::from(from.layout.title()),
            "ascii" => toml_edit::Value::from(from.ascii),
            "reduced_motion" => toml_edit::Value::from(from.reduced_motion),
            "no_splash" => toml_edit::Value::from(from.no_splash),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "unknown config setting",
                ));
            }
        };
        let mut document = self.document.clone();
        if let Some(old) = document.get(key).and_then(toml_edit::Item::as_value) {
            *value.decor_mut() = old.decor().clone();
        }
        document[key] = toml_edit::Item::Value(value);
        let body = document.to_string();
        let config = toml::from_str(&body)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let unchanged = || -> std::io::Result<()> {
            let current = match std::fs::read_to_string(path) {
                Ok(text) => Some(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            if current != self.source {
                return Err(std::io::Error::other(
                    "config changed on disk; reload bushel before saving settings",
                ));
            }
            Ok(())
        };
        unchanged()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        if let Ok(metadata) = std::fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() {
                return Err(std::io::Error::other(
                    "config is a symbolic link; edit it directly",
                ));
            }
            temporary
                .as_file()
                .set_permissions(metadata.permissions())?;
        }
        temporary.write_all(body.as_bytes())?;
        temporary.as_file().sync_all()?;
        unchanged()?;
        temporary.persist(path).map_err(|e| e.error)?;
        self.source = Some(body);
        self.document = document;
        self.config = config;
        Ok(path.clone())
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
        assert_eq!(
            Config::display_path(),
            "/tmp/bushel-config-test/config.toml"
        );
        unsafe { std::env::remove_var(Config::DIR_ENV) };
        assert_eq!(Config::display_path(), Config::DOC_PATH);
    }

    #[test]
    fn lossless_parser_limits_do_not_discard_successfully_loaded_settings() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("config.toml");
        let original = include_str!("../fixtures/config/uint64.toml");
        let effective =
            toml::from_str::<Config>(original).expect("the existing loader supports this file");
        assert!(effective.ascii);
        assert!(original.parse::<toml_edit::DocumentMut>().is_err());
        std::fs::write(&path, original).unwrap();
        let mut loaded = Config::load_from(path.clone());
        assert_eq!(
            loaded.effective(),
            effective,
            "lossless editing failure must not reset valid settings"
        );
        assert!(loaded.load_error().unwrap().contains("lossless"));
        assert!(
            loaded
                .save_setting("reduced_motion", &Config::default())
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn malformed_and_unreadable_configs_cannot_be_overwritten() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("config.toml");
        let original = "ascii = true\nlayout = [\n# preserve even malformed text\n";
        std::fs::write(&path, original).unwrap();
        let mut loaded = Config::load_from(path.clone());
        assert!(loaded.load_error().unwrap().contains("invalid config"));
        assert!(loaded.save_setting("ascii", &Config::default()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);

        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let mut loaded = Config::load_from(path.clone());
        assert!(
            loaded
                .load_error()
                .unwrap()
                .contains("could not read config")
        );
        assert!(loaded.save_setting("ascii", &Config::default()).is_err());
        assert!(path.is_dir());
    }

    #[test]
    fn external_config_edits_are_preserved_and_failed_saves_are_retryable() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("config.toml");
        let original = "ascii = false\n# keep\n";
        std::fs::write(&path, original).unwrap();
        let mut loaded = Config::load_from(path.clone());
        let changed = "ascii = false\nfuture = 123\n";
        std::fs::write(&path, changed).unwrap();
        let cfg = Config {
            ascii: true,
            ..Config::default()
        };
        assert!(
            loaded
                .save_setting("ascii", &cfg)
                .unwrap_err()
                .to_string()
                .contains("changed on disk")
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
        assert!(!loaded.effective().ascii);
        std::fs::write(&path, original).unwrap();
        loaded.save_setting("ascii", &cfg).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ascii = true\n# keep\n"
        );
        assert!(loaded.effective().ascii);
        assert_eq!(
            std::fs::read_dir(temporary.path()).unwrap().count(),
            1,
            "temporary file was renamed or removed"
        );
    }

    #[test]
    fn the_two_modes_cycle_into_each_other() {
        assert_eq!(LayoutMode::Rail.next(), LayoutMode::Table);
        assert_eq!(LayoutMode::Table.next(), LayoutMode::Rail);
    }
}
