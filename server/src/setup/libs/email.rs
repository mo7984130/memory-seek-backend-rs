use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Config {
    pub server: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
}
fn default_port() -> u16 {
    465
}

pub fn init(cfg: &Config) -> email::EmailClient {
    info!("初始化 Email 客户端");
    let client = email::EmailClient::new(
        &cfg.server,
        cfg.port,
        &cfg.username,
        &cfg.password,
        &cfg.from_email,
        &cfg.from_name,
    );
    info!("Email 客户端初始化成功");
    client
}
