use config::{Config, ConfigError, Environment, File};

use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    #[allow(dead_code)]
    pub smtp: SmtpConfig,
    #[cfg(feature = "s3")]
    pub s3: Option<oss::S3Config>,
    pub token_cipher: TokenCipherConfig,
    #[cfg(feature = "metrics")]
    pub metrics: Option<MetricsConfig>,
    #[cfg(feature = "backup")]
    pub backup: Option<backup::BackupConfig>,
    #[cfg(feature = "face-engine")]
    pub face_engine: FaceEngineConfig,
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
#[allow(dead_code)]
pub struct SmtpConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenCipherConfig {
    pub key: String,
    pub salt: String,
}

#[cfg(feature = "metrics")]
#[derive(Debug, Deserialize)]
pub struct MetricsConfig {
    pub host: String,
    pub port: u16,
}

#[cfg(feature = "face-engine")]
#[derive(Debug, Deserialize)]
pub struct FaceEngineConfig {
    pub detect_model_path: String,
    pub recognize_model_path: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        info!("加载配置文件");
        // 加载 .env 文件
        let _ = dotenvy::dotenv();

        let config_path =
            std::env::var("MEMORY_SEEK_CONFIG_PATH").unwrap_or_else(|_| "config.json".to_string());
        info!("配置文件路径: {}", config_path);

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
