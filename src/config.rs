use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct DatabaseConfig {
    pub postgres_url: Option<String>,
    pub redis_url: Option<String>,
    pub mongo_url: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: Option<String>,
    pub format: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: Some("info".into()),
            format: Some("pretty".into()),
        }
    }
}

impl Config {
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn merge_with_cli(
        self,
        postgres_url: Option<String>,
        redis_url: Option<String>,
        mongo_url: Option<String>,
    ) -> Self {
        Self {
            database: DatabaseConfig {
                postgres_url: postgres_url.or(self.database.postgres_url),
                redis_url: redis_url.or(self.database.redis_url),
                mongo_url: mongo_url.or(self.database.mongo_url),
            },
            logging: self.logging,
        }
    }

    pub fn default_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("dotconfig").join("config.toml"))
    }

    pub fn load_or_default(config_path: Option<String>) -> Self {
        if let Some(path) = config_path {
            if let Ok(config) = Self::load_from_file(&path) {
                tracing::info!("Loaded config from {}", path);
                return config;
            } else {
                tracing::warn!("Failed to load config from {}, using defaults", path);
            }
        } else if let Some(default_path) = Self::default_config_path() {
            if default_path.exists() {
                if let Ok(config) = Self::load_from_file(default_path.to_str().unwrap()) {
                    tracing::info!("Loaded config from {:?}", default_path);
                    return config;
                }
            }
        }

        Self::default()
    }
}
