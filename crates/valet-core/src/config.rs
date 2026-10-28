use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub inbox_paths: Vec<String>,
    pub quarantine_retention_days: u32,
    pub tag_suggestions: Vec<String>,
    pub always_do_actions: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            inbox_paths: vec![],
            quarantine_retention_days: 30,
            tag_suggestions: vec!["work".into(), "personal".into(), "invoice".into()],
            always_do_actions: false,
        }
    }
}

/// Tauri bundle identifier; also the name of the config folder.
pub const APP_IDENTIFIER: &str = "ro.valetapp.desktop";

/// Folder for `config.json`, `app_settings.json` and `valet.sqlite3`.
/// Equals Tauri's `app_config_dir()` (`%APPDATA%\ro.valetapp.desktop` on Windows).
pub fn config_dir() -> Result<PathBuf> {
    let base = BaseDirs::new().context("cannot determine config directory")?;
    Ok(base.config_dir().join(APP_IDENTIFIER))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        let cfg = Config::default();
        save(&cfg)?;
        return Ok(cfg);
    }
    let bytes = fs::read(&path)?;
    let cfg: Config = serde_json::from_slice(&bytes)?;
    Ok(cfg)
}

pub fn save(cfg: &Config) -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)?;
    fs::write(config_path()?, serde_json::to_vec_pretty(cfg)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_ends_with_app_identifier() {
        let dir = config_dir().unwrap();
        assert_eq!(dir.file_name().unwrap(), APP_IDENTIFIER);
        assert_eq!(dir.parent().unwrap(), BaseDirs::new().unwrap().config_dir());
    }
}
