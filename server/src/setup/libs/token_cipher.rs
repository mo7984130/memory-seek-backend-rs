use common::utils::TokenCipherConfig;
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub key: String,
    pub salt: String,
}

/// 根据配置初始化全局令牌加密器.
pub fn init(cfg: &Config) {
    info!("初始化 TokenCipher");
    common::utils::init_token_cipher(&TokenCipherConfig {
        key: cfg.key.clone(),
        salt: cfg.salt.clone(),
    });
    info!("TokenCipher 初始化成功");
}
