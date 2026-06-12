use std::sync::Arc;
use common::utils::TokenCipher;
use crate::config::AppConfig;

pub fn init(cfg: &AppConfig) -> anyhow::Result<Arc<TokenCipher>> {
    let cipher = TokenCipher::from_config(&common::utils::TokenCipherConfig {
        key: cfg.token_cipher.key.clone(),
        salt: cfg.token_cipher.salt.clone(),
    });
    tracing::info!("TokenCipher initialized");
    Ok(Arc::new(cipher))
}
