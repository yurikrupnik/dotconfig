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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_default() {
        let config = Config::default();

        assert!(config.database.postgres_url.is_none());
        assert!(config.database.redis_url.is_none());
        assert!(config.database.mongo_url.is_none());
        assert_eq!(config.logging.level, Some("info".into()));
        assert_eq!(config.logging.format, Some("pretty".into()));
    }

    #[test]
    fn test_database_config_default() {
        let db_config = DatabaseConfig::default();

        assert!(db_config.postgres_url.is_none());
        assert!(db_config.redis_url.is_none());
        assert!(db_config.mongo_url.is_none());
    }

    #[test]
    fn test_logging_config_default() {
        let logging_config = LoggingConfig::default();

        assert_eq!(logging_config.level, Some("info".into()));
        assert_eq!(logging_config.format, Some("pretty".into()));
    }

    #[test]
    fn test_load_from_valid_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[database]
postgres_url = "postgres://localhost/test"
redis_url = "redis://localhost:6379"

[logging]
level = "debug"
format = "json"
"#;
        temp_file.write_all(toml_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let config = Config::load_from_file(temp_file.path().to_str().unwrap()).unwrap();

        assert_eq!(
            config.database.postgres_url,
            Some("postgres://localhost/test".into())
        );
        assert_eq!(config.database.redis_url, Some("redis://localhost:6379".into()));
        assert_eq!(config.logging.level, Some("debug".into()));
        assert_eq!(config.logging.format, Some("json".into()));
    }

    #[test]
    fn test_load_from_invalid_file() {
        let result = Config::load_from_file("nonexistent.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_malformed_toml() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let invalid_toml = "this is not valid toml [[[";
        temp_file.write_all(invalid_toml.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let result = Config::load_from_file(temp_file.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_with_cli_override_all() {
        let base_config = Config {
            database: DatabaseConfig {
                postgres_url: Some("postgres://base".into()),
                redis_url: Some("redis://base".into()),
                mongo_url: Some("mongo://base".into()),
            },
            logging: LoggingConfig {
                level: Some("info".into()),
                format: Some("pretty".into()),
            },
        };

        let merged = base_config.merge_with_cli(
            Some("postgres://cli".into()),
            Some("redis://cli".into()),
            Some("mongo://cli".into()),
        );

        assert_eq!(merged.database.postgres_url, Some("postgres://cli".into()));
        assert_eq!(merged.database.redis_url, Some("redis://cli".into()));
        assert_eq!(merged.database.mongo_url, Some("mongo://cli".into()));
        assert_eq!(merged.logging.level, Some("info".into()));
        assert_eq!(merged.logging.format, Some("pretty".into()));
    }

    #[test]
    fn test_merge_with_cli_partial_override() {
        let base_config = Config {
            database: DatabaseConfig {
                postgres_url: Some("postgres://base".into()),
                redis_url: Some("redis://base".into()),
                mongo_url: Some("mongo://base".into()),
            },
            logging: LoggingConfig {
                level: Some("info".into()),
                format: Some("pretty".into()),
            },
        };

        let merged = base_config.merge_with_cli(Some("postgres://cli".into()), None, None);

        assert_eq!(merged.database.postgres_url, Some("postgres://cli".into()));
        assert_eq!(merged.database.redis_url, Some("redis://base".into()));
        assert_eq!(merged.database.mongo_url, Some("mongo://base".into()));
    }

    #[test]
    fn test_merge_with_cli_no_override() {
        let base_config = Config {
            database: DatabaseConfig {
                postgres_url: Some("postgres://base".into()),
                redis_url: Some("redis://base".into()),
                mongo_url: Some("mongo://base".into()),
            },
            logging: LoggingConfig {
                level: Some("info".into()),
                format: Some("pretty".into()),
            },
        };

        let merged = base_config.merge_with_cli(None, None, None);

        assert_eq!(
            merged.database.postgres_url,
            Some("postgres://base".into())
        );
        assert_eq!(merged.database.redis_url, Some("redis://base".into()));
        assert_eq!(merged.database.mongo_url, Some("mongo://base".into()));
    }

    #[test]
    fn test_merge_with_cli_empty_base() {
        let base_config = Config::default();

        let merged = base_config.merge_with_cli(
            Some("postgres://cli".into()),
            Some("redis://cli".into()),
            Some("mongo://cli".into()),
        );

        assert_eq!(merged.database.postgres_url, Some("postgres://cli".into()));
        assert_eq!(merged.database.redis_url, Some("redis://cli".into()));
        assert_eq!(merged.database.mongo_url, Some("mongo://cli".into()));
    }

    #[test]
    fn test_load_or_default_with_invalid_path() {
        let config = Config::load_or_default(Some("nonexistent.toml".into()));
        assert!(config.database.postgres_url.is_none());
    }

    #[test]
    fn test_load_or_default_with_valid_path() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[database]
postgres_url = "postgres://test"
"#;
        temp_file.write_all(toml_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let config = Config::load_or_default(Some(temp_file.path().to_str().unwrap().into()));

        assert_eq!(
            config.database.postgres_url,
            Some("postgres://test".into())
        );
    }

    #[test]
    fn test_load_or_default_none() {
        let config = Config::load_or_default(None);
        assert!(config.database.postgres_url.is_none());
    }

    #[test]
    fn test_config_cloneable() {
        let config = Config::default();
        let cloned = config.clone();

        assert_eq!(
            config.database.postgres_url,
            cloned.database.postgres_url
        );
        assert_eq!(config.logging.level, cloned.logging.level);
    }
}
