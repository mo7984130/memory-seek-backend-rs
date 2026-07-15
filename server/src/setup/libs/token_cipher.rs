use crate::config::AppConfig;
use common::utils::TokenCipher;
use std::sync::Arc;
use tracing::info;

pub fn init(cfg: &AppConfig) -> Arc<TokenCipher> {
    info!("初始化 TokenCipher");
    let cipher = TokenCipher::from_config(&common::utils::TokenCipherConfig {
        key: cfg.token_cipher.key.clone(),
        salt: cfg.token_cipher.salt.clone(),
    });
    info!("TokenCipher 初始化成功");
    Arc::new(cipher)
}
