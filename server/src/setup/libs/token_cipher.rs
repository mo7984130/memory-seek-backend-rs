use common_crypto::TokenCipherConfig;
use serde::Deserialize;
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub key: String,
    pub salt: String,
}

#[common_macros::register_async(
    slice = crate::setup::libs::APP_LIBS,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, _setup: &mut AppSetup) -> common_core::Result<()> {
    debug!("初始化 TokenCipher lib");
    let config = &config.token_cipher;
    common_crypto::init_token_cipher(&TokenCipherConfig {
        key: config.key.clone(),
        salt: config.salt.clone(),
    });
    info!("初始化 TokenCipher lib 成功");
    Ok(())
}
