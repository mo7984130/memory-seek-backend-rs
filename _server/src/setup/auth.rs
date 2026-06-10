use std::sync::Arc;

use auth::{AuthState, controller::AuthController};
use axum::Router;
use common::utils::TokenCipher;
use deadpool_redis::Pool;
use email::EmailClient;
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::config::AppConfig;

#[derive(Clone, Deserialize)]
pub struct SmtpConfig {
    pub server: String,
    pub port: u16,
    pub password: String,
    pub username: String,
    pub from_email: String,
    pub from_name: String,
}

/// 初始化认证模块
///
/// 根据应用配置创建邮件客户端和认证状态。
///
/// # 参数
/// - `cfg`: 应用配置，包含 SMTP 邮件配置
/// - `db`: 数据库连接
/// - `redis`: Redis 连接池
/// - `token_cipher`: Token 加解密器
///
/// # 返回
/// 返回封装好的认证状态 `Arc<AuthState>`
pub fn init_auth(
    cfg: &AppConfig,
    db: DatabaseConnection,
    redis: Pool,
    token_cipher: Arc<TokenCipher>
) -> Arc<AuthState> {
    let smtp = &cfg.smtp_config;
    let email_client = EmailClient::new(
        &smtp.server,
        smtp.port,
        &smtp.username,
        &smtp.password,
        &smtp.from_email,
        &smtp.from_name,
    );

    Arc::new(AuthState::new(db, redis, email_client, token_cipher))
}

/// 挂载认证模块的公开路由
///
/// 将认证相关的公开路由（如登录、注册等）挂载到 `/auth` 路径下。
///
/// # 参数
/// - `router`: 已有的路由
/// - `state`: 认证状态
///
/// # 返回
/// 返回挂载了认证路由的新路由
pub fn mount_public(
    router: Router,
    state: Arc<AuthState>
) -> Router {
     router.nest("/auth", AuthController::routes().with_state(state))
}
