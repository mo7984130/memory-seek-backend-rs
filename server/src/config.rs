use serde::Deserialize;
use config::{Config, ConfigError, Environment, File};

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub smtp: SmtpConfig,
    pub s3: Option<S3Config>,
    pub token_cipher: TokenCipherConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize)]
pub struct SmtpConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
}

#[derive(Debug, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub public_url: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenCipherConfig {
    pub key: String,
    pub salt: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        // 加载 .env 文件
        let _ = dotenvy::dotenv();

        let config_path = std::env::var("MEMORY_SEEK_CONFIG_PATH")
            .unwrap_or_else(|_| "config.json".to_string());

        let cfg = Config::builder()
            .add_source(File::with_name(&config_path))
            .add_source(Environment::with_prefix("MEMORY_SEEK"))
            .build()?;

        cfg.try_deserialize()
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}
