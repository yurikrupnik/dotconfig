use anyhow::Result;

const DEFAULT_NEO4J_URI: &str = "bolt://localhost:7687";
const DEFAULT_NEO4J_USERNAME: &str = "neo4j";
const DEFAULT_NEO4J_PASSWORD: &str = "password";

#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    pub postgres: Option<PostgresPool>,
    #[allow(dead_code)]
    pub redis: Option<RedisPool>,
    #[allow(dead_code)]
    pub mongo: Option<MongoPool>,
    pub neo4j_uri: String,
    pub neo4j_username: String,
    pub neo4j_password: String,
}

pub type PostgresPool = ();
pub type RedisPool = ();
pub type MongoPool = ();

impl AppState {
    pub fn new() -> Self {
        Self {
            postgres: None,
            redis: None,
            mongo: None,
            neo4j_uri: DEFAULT_NEO4J_URI.into(),
            neo4j_username: DEFAULT_NEO4J_USERNAME.into(),
            neo4j_password: DEFAULT_NEO4J_PASSWORD.into(),
        }
    }

    pub async fn with_databases(
        _postgres_url: Option<String>,
        _redis_url: Option<String>,
        _mongo_url: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            postgres: None,
            redis: None,
            mongo: None,
            neo4j_uri: std::env::var("NEO4J_URI").unwrap_or_else(|_| DEFAULT_NEO4J_URI.into()),
            neo4j_username: std::env::var("NEO4J_USERNAME")
                .unwrap_or_else(|_| DEFAULT_NEO4J_USERNAME.into()),
            neo4j_password: std::env::var("NEO4J_PASSWORD")
                .unwrap_or_else(|_| DEFAULT_NEO4J_PASSWORD.into()),
        })
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();

        assert!(state.postgres.is_none());
        assert!(state.redis.is_none());
        assert!(state.mongo.is_none());
        assert_eq!(state.neo4j_uri, DEFAULT_NEO4J_URI);
        assert_eq!(state.neo4j_username, DEFAULT_NEO4J_USERNAME);
        assert_eq!(state.neo4j_password, DEFAULT_NEO4J_PASSWORD);
    }

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();

        assert!(state.postgres.is_none());
        assert!(state.redis.is_none());
        assert!(state.mongo.is_none());
        assert_eq!(state.neo4j_uri, DEFAULT_NEO4J_URI);
        assert_eq!(state.neo4j_username, DEFAULT_NEO4J_USERNAME);
        assert_eq!(state.neo4j_password, DEFAULT_NEO4J_PASSWORD);
    }

    #[tokio::test]
    async fn test_with_databases_ignores_params() {
        let result = AppState::with_databases(
            Some("postgres://test".into()),
            Some("redis://test".into()),
            Some("mongo://test".into()),
        )
        .await;

        assert!(result.is_ok());
        let state = result.unwrap();

        assert!(state.postgres.is_none());
        assert!(state.redis.is_none());
        assert!(state.mongo.is_none());
    }

    #[tokio::test]
    async fn test_with_databases_uses_env_vars() {
        std::env::set_var("NEO4J_URI", "bolt://test:7687");
        std::env::set_var("NEO4J_USERNAME", "testuser");
        std::env::set_var("NEO4J_PASSWORD", "testpass");

        let result = AppState::with_databases(None, None, None).await;
        assert!(result.is_ok());

        let state = result.unwrap();
        assert_eq!(state.neo4j_uri, "bolt://test:7687");
        assert_eq!(state.neo4j_username, "testuser");
        assert_eq!(state.neo4j_password, "testpass");

        std::env::remove_var("NEO4J_URI");
        std::env::remove_var("NEO4J_USERNAME");
        std::env::remove_var("NEO4J_PASSWORD");
    }

    #[tokio::test]
    async fn test_with_databases_defaults() {
        std::env::remove_var("NEO4J_URI");
        std::env::remove_var("NEO4J_USERNAME");
        std::env::remove_var("NEO4J_PASSWORD");

        let result = AppState::with_databases(None, None, None).await;
        assert!(result.is_ok());

        let state = result.unwrap();
        assert_eq!(state.neo4j_uri, DEFAULT_NEO4J_URI);
        assert_eq!(state.neo4j_username, DEFAULT_NEO4J_USERNAME);
        assert_eq!(state.neo4j_password, DEFAULT_NEO4J_PASSWORD);
    }

    #[test]
    fn test_constants_values() {
        assert_eq!(DEFAULT_NEO4J_URI, "bolt://localhost:7687");
        assert_eq!(DEFAULT_NEO4J_USERNAME, "neo4j");
        assert_eq!(DEFAULT_NEO4J_PASSWORD, "password");
    }

    #[test]
    fn test_state_cloneable() {
        let state = AppState::new();
        let cloned = state.clone();

        assert_eq!(state.neo4j_uri, cloned.neo4j_uri);
        assert_eq!(state.neo4j_username, cloned.neo4j_username);
        assert_eq!(state.neo4j_password, cloned.neo4j_password);
    }
}
