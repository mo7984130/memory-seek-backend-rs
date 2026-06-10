use common::utils::TokenCipherConfig;
use serde::Deserialize;

use crate::setup::{database::DatabaseConfig, redis::RedisConfig};

#[derive(Clone, Deserialize)]
pub struct AppConfig {
    pub database_config: DatabaseConfig,
    pub redis_config: RedisConfig,
    #[cfg(feature = "auth")]
    pub smtp_config: crate::setup::auth::SmtpConfig,
    #[cfg(any(feature = "photo", feature = "user"))]
    pub oss_config: oss::S3Config,
    pub token_cipher_config: TokenCipherConfig,
    #[cfg(feature = "face_recognition")]
    pub face_recognition_config: crate::setup::photo::FaceRecognitionConfig,
}

impl AppConfig {
    /// 从环境变量加载应用配置
    ///
    /// 加载 `.env` 文件（如果存在），然后从环境变量中读取配置。
    /// 环境变量使用 `__` 作为嵌套分隔符。
    ///
    /// # 返回
    /// 返回反序列化后的 `AppConfig` 实例
    ///
    /// # 错误
    /// - 构建配置失败时 panic
    /// - 配置反序列化失败时 panic
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let s = config::Config::builder()
            .add_source(config::Environment::default().separator("__"))
            .build()
            .expect("构建配置失败");

        s.try_deserialize().expect("配置反序列化失败")
    }

    /// 从 JSON 文件加载应用配置
    ///
    /// 从指定路径的 JSON 文件中读取配置并反序列化为 `AppConfig`。
    ///
    /// # 参数
    /// - `path`: JSON 配置文件的路径
    ///
    /// # 返回
    /// 返回反序列化后的 `AppConfig` 实例
    ///
    /// # 错误
    /// - 构建配置失败时 panic
    /// - 配置反序列化失败时 panic
    #[allow(dead_code)]
    pub fn from_json(path: &str) -> Self {
        let s = config::Config::builder()
            .add_source(config::File::new(path, config::FileFormat::Json))
            .build()
            .expect("构建配置失败");

        s.try_deserialize().expect("配置反序列化失败")
    }
}
