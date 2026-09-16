//! e2e 配置加载, 读取逻辑与 server 一致:
//! 1. CLI 参数 `--config` / `-c`
//! 2. 环境变量 `E2E_CONFIG_PATH`
//! 3. 默认值 `e2e.config.yml`(运行目录)或 `tests/e2e.config.yml`
//!
//! 所有配置项均无默认值, 必须显式提供; 环境变量(前缀 `E2E`, 分隔符 `__`)可覆盖文件配置,
//! 如 `E2E__SEED__AUTH_USERS=100`。

use std::path::PathBuf;

use config::{Config, Environment, File};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct E2eConfig {
    pub server: ServerConfig,

    pub database: DatabaseConfig,

    pub redis: RedisConfig,

    pub mailhog: MailhogConfig,

    /// 对象存储(头像回查), 需与 server 运行时 s3 段一致
    pub s3: S3Config,

    /// 图片 token 加密配置, 必须与 server 的 token_cipher 一致
    pub token_cipher: TokenCipherConfig,

    pub seed: SeedConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// 被测 server 的基础地址, 可含路径前缀(如 `https://example.com/api`)
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct MailhogConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
    pub public_url: Option<String>,
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
    pub auth_users: u64,
    pub media_users: u64,
    pub medias_per_user: u64,
    pub faces_per_person: u64,
    /// user 模块专属测试用户池大小(uit_user_* / uit_pwd_*), 需 >= Manager 并发度
    pub uit_users: u64,
}

impl SeedConfig {
    pub fn media_count(&self) -> u64 {
        self.media_users * self.medias_per_user
    }
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

    /// 返回被测 server 的访问地址(去掉末尾 `/`, 便于拼接请求路径).
    pub fn base_url(&self) -> String {
        self.server.url.trim_end_matches('/').to_string()
    }
}
