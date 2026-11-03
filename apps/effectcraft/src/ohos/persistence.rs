//! Ability-local settings, workspaces and recovery, through the engine's existing stores.

use effectcraft_engine::config::{ConfigStore, DirConfig};
use std::path::{Component, Path, PathBuf};

const MAX_CONFIG_BYTES: usize = 4 * 1024 * 1024;

pub struct Config(DirConfig);

impl Config {
    pub fn new(directory: PathBuf) -> Self {
        Self(DirConfig::new(directory))
    }
}

fn entry(name: &str) -> std::io::Result<&str> {
    if name.is_empty() || Path::new(name).components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "configuration entry must be a relative path within the preferences directory"));
    }
    Ok(match name {
        "prefs.json" => "settings.json",
        "workspaces.json" => "ui.json",
        "workspaces.old.json" => "ui.old.json",
        other => other,
    })
}

impl ConfigStore for Config {
    fn read(&self, name: &str) -> Option<String> {
        match self.read_bounded(name, MAX_CONFIG_BYTES) {
            Ok(text) => {
                if name == "prefs.json"
                    && let Some(text) = &text
                    && let Err(error) = serde_json::from_str::<serde_json::Value>(text)
                {
                    log::warn!("Cannot parse settings.json: {error}; using default preferences");
                }
                text
            }
            Err(error) => {
                log::warn!("Could not read {name}: {error}; using defaults");
                None
            }
        }
    }

    fn read_bounded(&self, name: &str, max_bytes: usize) -> std::io::Result<Option<String>> {
        self.0.read_bounded(entry(name)?, max_bytes.min(MAX_CONFIG_BYTES))
    }

    fn write(&self, name: &str, data: &str) -> std::io::Result<()> {
        self.0.write(entry(name)?, data)
    }

    fn remove(&self, name: &str) -> std::io::Result<()> {
        self.0.remove(entry(name)?)
    }

    fn list(&self, dir: &str) -> Vec<String> {
        entry(dir).map(|dir| self.0.list(dir)).unwrap_or_default()
    }

    fn list_dirs(&self, dir: &str) -> Vec<String> {
        entry(dir).map(|dir| self.0.list_dirs(dir)).unwrap_or_default()
    }

    fn dir(&self) -> Option<PathBuf> {
        self.0.dir()
    }
}
