use common::utils::TokenCipherConfig;
use serde::Deserialize;
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub key: String,
    pub salt: String,
}

#[common::register_async(
    slice = crate::setup::libs::APP_LIBS,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, _setup: &mut AppSetup) -> common::Result<()> {
    debug!("初始化 TokenCipher lib");
    let config = &config.token_cipher;
    common::utils::init_token_cipher(&TokenCipherConfig {
        key: config.key.clone(),
        salt: config.salt.clone(),
    });
    info!("初始化 TokenCipher lib 成功");
    Ok(())
}
