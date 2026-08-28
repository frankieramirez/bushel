use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub no_splash: bool,
    pub reduced_motion: bool,
    pub ascii: bool,
}

impl Config {
    pub const DOC_PATH: &'static str = "~/.config/bushel/config.toml";

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
