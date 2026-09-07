use oss::S3Client;
use serde::Deserialize;
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
    pub public_url: Option<String>,
    #[serde(default)]
    pub force_path_style: bool,
}

#[common::register_async(
    slice = crate::setup::libs::APP_LIBS,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> common::Result<()> {
    debug!("初始化 S3Client lib");
    let config = &config.s3;
    let client = S3Client::new(&config.to_oss_config());
    setup.registry.insert(client);
    info!("初始化 S3Client lib 成功");
    Ok(())
}

impl Config {
    /// 将应用配置转换为对象存储客户端配置.
    pub fn to_oss_config(&self) -> oss::S3Config {
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
