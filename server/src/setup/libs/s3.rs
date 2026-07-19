use oss::S3Client;
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

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

pub fn init(cfg: &Config) -> Arc<S3Client> {
    info!("初始化 S3 客户端");
    let client = S3Client::new(&cfg.to_oss_config());
    info!("S3 客户端初始化成功");
    Arc::new(client)
}

impl Config {
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
