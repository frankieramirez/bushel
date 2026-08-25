//! `~/.config/bushel/config.toml` (or the platform config dir): the config file
//! only holds what flags can also set.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub no_splash: bool,
    pub reduced_motion: bool,
    pub ascii: bool,
}

impl Config {
    pub fn load() -> Self {
        let Some(dir) = dirs::config_dir() else {
            return Self::default();
        };
        let path = dir.join("bushel").join("config.toml");
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
}
