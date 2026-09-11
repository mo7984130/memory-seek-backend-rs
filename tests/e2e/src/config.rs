//! e2e 配置加载, 读取逻辑与 server 一致:
//! 1. CLI 参数 `--config` / `-c`
//! 2. 环境变量 `E2E_CONFIG_PATH`
//! 3. 默认值 `e2e.config.yml`(运行目录)或 `tests/e2e.config.yml`
//!
//! 环境变量(前缀 `E2E`, 分隔符 `__`)可覆盖文件配置, 如 `E2E__SEED__AUTH_USERS=100`。

use std::path::PathBuf;

use config::{Config, Environment, File};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct E2eConfig {
    #[serde(default)]
    pub server: ServerConfig,

    pub database: DatabaseConfig,

    #[serde(default)]
    pub redis: RedisConfig,

    #[serde(default)]
    pub mailhog: MailhogConfig,

    /// 对象存储(头像回查), 需与 server 运行时 s3 段一致
    pub s3: S3Config,

    /// 图片 token 加密配置, 必须与 server 的 token_cipher 一致
    pub token_cipher: TokenCipherConfig,

    #[serde(default)]
    pub seed: SeedConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
        }
    }
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

#[derive(Debug, Deserialize)]
pub struct MailhogConfig {
    #[serde(default = "default_mailhog_url")]
    pub url: String,
}

impl Default for MailhogConfig {
    fn default() -> Self {
        Self {
            url: default_mailhog_url(),
        }
    }
}

fn default_mailhog_url() -> String {
    "http://localhost:8025".to_string()
}

#[derive(Debug, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
    pub public_url: Option<String>,
    #[serde(default)]
    pub force_path_style: bool,
}

impl S3Config {
    /// 转换为 `oss` 库的客户端配置.
    pub fn to_oss(&self) -> oss::S3Config {
        oss::S3Config {
            endpoint: self.endpoint.clone(),
            access_key: self.access_key.clone(),
            secret_key: self.secret_key.clone(),
            region: self.region.clone(),
            bucket: self.bucket.clone(),
            public_url: self.public_url.clone(),
            force_path_style: self.force_path_style,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenCipherConfig {
    pub key: String,
    pub salt: String,
}

#[derive(Debug, Deserialize)]
pub struct SeedConfig {
    #[serde(default = "default_auth_users")]
    pub auth_users: u64,
    #[serde(default = "default_photo_users")]
    pub photo_users: u64,
    #[serde(default = "default_photos_per_user")]
    pub photos_per_user: u64,
    #[serde(default = "default_faces_per_person")]
    pub faces_per_person: u64,
    /// user 模块专属测试用户池大小(uit_user_* / uit_pwd_*), 需 >= Manager 并发度
    #[serde(default = "default_uit_users")]
    pub uit_users: u64,
}

impl Default for SeedConfig {
    fn default() -> Self {
        Self {
            auth_users: default_auth_users(),
            photo_users: default_photo_users(),
            photos_per_user: default_photos_per_user(),
            faces_per_person: default_faces_per_person(),
            uit_users: default_uit_users(),
        }
    }
}

impl SeedConfig {
    pub fn photo_count(&self) -> u64 {
        self.photo_users * self.photos_per_user
    }
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
const fn default_port() -> u16 {
    7985
}
const fn default_auth_users() -> u64 {
    10_000
}
const fn default_photo_users() -> u64 {
    2_000
}
const fn default_photos_per_user() -> u64 {
    20
}
const fn default_faces_per_person() -> u64 {
    5
}
const fn default_uit_users() -> u64 {
    32
}

impl E2eConfig {
    /// 加载配置, 按优先级确定配置文件路径:
    /// 1. CLI 参数 `--config` / `-c`
    /// 2. 环境变量 `E2E_CONFIG_PATH`
    /// 3. 默认值 `e2e.config.yml`(运行目录)或 `tests/e2e.config.yml`
    pub fn load(cli_config_path: Option<String>) -> Self {
        info!("加载配置文件");

        let config_path = if let Some(path) = cli_config_path {
            PathBuf::from(path)
        } else if let Ok(path) = std::env::var("E2E_CONFIG_PATH") {
            PathBuf::from(path)
        } else {
            // 依次探测: 运行目录 > 仓库根 tests/e2e/ 下的默认配置
            [
                PathBuf::from("e2e.config.yml"),
                PathBuf::from("tests/e2e.config.yml"),
                PathBuf::from("tests/e2e/e2e.config.yml"),
            ]
            .into_iter()
            .find(|path| path.exists())
            .unwrap_or_else(|| PathBuf::from("tests/e2e/e2e.config.yml"))
        };

        info!("配置文件路径: {:?}", config_path);

        let cfg = Config::builder()
            .add_source(File::from(config_path))
            .add_source(Environment::with_prefix("E2E").separator("__"))
            .build()
            .expect("构建配置失败");

        cfg.try_deserialize().expect("反序列化配置失败")
    }

    /// 返回被测 server 的访问地址.
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.server.host, self.server.port)
    }
}
