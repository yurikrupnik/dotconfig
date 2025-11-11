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
            neo4j_uri: std::env::var("NEO4J_URI")
                .unwrap_or_else(|_| DEFAULT_NEO4J_URI.into()),
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
