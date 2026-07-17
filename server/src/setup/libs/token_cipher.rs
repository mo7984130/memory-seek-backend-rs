use common::utils::TokenCipher;
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub key: String,
    pub salt: String,
}

pub fn init(cfg: &Config) -> Arc<TokenCipher> {
    info!("初始化 TokenCipher");
    let cipher = TokenCipher::from_config(&common::utils::TokenCipherConfig {
        key: cfg.key.clone(),
        salt: cfg.salt.clone(),
    });
    info!("TokenCipher 初始化成功");
    Arc::new(cipher)
}
