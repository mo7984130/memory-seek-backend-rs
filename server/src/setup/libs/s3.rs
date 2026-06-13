use crate::config::AppConfig;
use oss::{S3Client, S3Config};
use std::sync::Arc;
use tracing::info;

pub fn init(cfg: &AppConfig) -> anyhow::Result<Arc<S3Client>> {
    info!("初始化 S3 客户端");
    let s3 = cfg.s3.as_ref().unwrap();
    let oss_config = S3Config {
        endpoint: s3.endpoint.clone(),
        access_key: s3.access_key.clone(),
        secret_key: s3.secret_key.clone(),
        region: s3.region.clone(),
        bucket: s3.bucket.clone(),
        public_url: s3.public_url.clone(),
        force_path_style: false,
    };
    let client = S3Client::new(&oss_config);
    info!("S3 客户端初始化成功");
    Ok(Arc::new(client))
}
