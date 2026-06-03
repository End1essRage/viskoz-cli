use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
 
const CONFIG_FILE: &str = "runner-cli.toml";
 
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub cp_address: String,
    pub mesh_ip: String,
    pub runner_token: String,
}
 
impl Config {
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        let content = toml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
 
    pub fn load() -> Result<Self> {
        let path = config_path();
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
 
fn config_path() -> PathBuf {
    // ~/.runner-cli/runner-cli.toml
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".runner-cli")
        .join(CONFIG_FILE)
}
 
