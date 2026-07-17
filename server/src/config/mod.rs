use config::{Config, ConfigError, Environment, File};

use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: crate::setup::bases::database::Config,
    pub redis: crate::setup::bases::redis::Config,
    #[allow(dead_code)]
    pub smtp: crate::setup::libs::email::Config,
    #[cfg(feature = "s3")]
    pub s3: Option<crate::setup::libs::s3::Config>,
    pub token_cipher: crate::setup::libs::token_cipher::Config,
    #[serde(default)]
    pub log: crate::setup::bases::log::Config,
    #[cfg(feature = "metrics")]
    pub metrics: Option<crate::setup::bases::metrics::Config>,
    #[cfg(feature = "backup")]
    pub backup: Option<crate::setup::domains::backup::Config>,
    #[cfg(feature = "face-engine")]
    pub face_engine: crate::setup::libs::face_engine::Config,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}
fn default_host() -> String {
    "127.0.0.1".to_string()
}
const fn default_port() -> u16 {
    7984
}

impl AppConfig {
    /// 加载配置，按优先级确定配置文件路径：
    /// 1. CLI 参数 `--config` / `-c`
    /// 2. 环境变量 `MEMORY_SEEK_CONFIG_PATH`
    /// 3. 默认值 `config.yaml`
    pub fn load(cli_config_path: Option<String>) -> Result<Self, ConfigError> {
        info!("加载配置文件");
        let _ = dotenvy::dotenv();

        let config_path = cli_config_path
            .or_else(|| std::env::var("MEMORY_SEEK_CONFIG_PATH").ok())
            .unwrap_or_else(|| "config.yml".to_string());
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
