use crate::pbs::AuthConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

fn default_task_url_prefix() -> String {
    "https://pbs2.praguebest.cz/main.php?pageid=110&action=detail&id=".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub auth: AuthConfig,
    #[serde(default = "default_task_url_prefix")]
    pub task_url_prefix: String,
}

impl Config {
    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
