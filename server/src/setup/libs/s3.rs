use crate::config::AppConfig;
use oss::S3Client;
use std::sync::Arc;
use tracing::info;

pub fn init(cfg: &AppConfig) -> Arc<S3Client> {
    info!("初始化 S3 客户端");
    let client = S3Client::new(&cfg.s3.as_ref().expect("未配置s3"));
    info!("S3 客户端初始化成功");
    Arc::new(client)
}
