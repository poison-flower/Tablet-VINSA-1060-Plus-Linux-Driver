use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

const APP_QUALIFIER: &str = "com";
const APP_ORG: &str = "theninth";
const APP_NAME: &str = "v1060p-driver";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub pressure_threshold: u16,
    pub sensitivity: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            pressure_threshold: 510,
            sensitivity: 5.0,
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let config_path = Self::get_config_path();
        if config_path.exists() {
            let content = fs::read_to_string(config_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_else(|_| Self::default())
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let config_path = Self::get_config_path();
        
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    pub fn get_config_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from(APP_QUALIFIER, APP_ORG, APP_NAME) {
            proj_dirs.config_dir().join("settings.json")
        } else {
            PathBuf::from("settings.json")
        }
    }
}
