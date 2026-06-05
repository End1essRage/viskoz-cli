use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CONFIG_FILE: &str = "mgs-cli.toml";

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub mesh_ip: String,
    pub cp_runner_addr: String,
}

impl Config {
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        
        // Создаем родительскую директорию, если она не существует
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }
        
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config to TOML")?;
        
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write config to {:?}", path))?;
        
        tracing::info!("Config saved to {:?}", path);
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let path = config_path();
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {:?}", path))?;
        
        let config: Config = toml::from_str(&content)
            .context("Failed to parse config TOML")?;
        
        Ok(config)
    }
}

fn config_path() -> PathBuf {
    // ~/.mgs-cli/mgs-cli.toml
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mgs-cli")
        .join(CONFIG_FILE)
}