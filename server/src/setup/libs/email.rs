use serde::Deserialize;
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
}
/// 返回 SMTP 默认端口.
fn default_port() -> u16 {
    465
}

#[common::register_async(
    slice = crate::setup::libs::APP_LIBS,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> common::Result<()> {
    debug!("初始化 Email Client");
    let config = &config.smtp;
    let client = email::EmailClient::new(
        &config.server,
        config.port,
        &config.username,
        &config.password,
        &config.from_email,
        &config.from_name,
    );
    setup.registry.insert(client);
    info!("初始化 Email Client lib 成功");
    Ok(())
}
