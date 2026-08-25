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
    /// `~/.config/bushel/` (as documented) with the platform config dir as a
    /// fallback — on macOS `dirs::config_dir()` is `~/Library/Application
    /// Support`, which nobody expects a CLI tool to use.
    pub fn dir() -> Option<std::path::PathBuf> {
        if let Some(home) = dirs::home_dir() {
            let xdg = home.join(".config").join("bushel");
            return Some(xdg);
        }
        dirs::config_dir().map(|d| d.join("bushel"))
    }

    pub fn load() -> Self {
        let Some(dir) = Self::dir() else {
            return Self::default();
        };
        let path = dir.join("config.toml");
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
