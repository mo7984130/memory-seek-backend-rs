use std::sync::Arc;

use axum::Router;
use common::utils::TokenCipher;
use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;
use user::controller::UserController;
use user::UserState;

use crate::config::AppConfig;

/// 初始化用户模块
///
/// 创建用户模块状态，注入数据库、Redis、OSS 客户端和 Token 加解密器。
///
/// # 参数
/// - `_cfg`: 应用配置（当前未使用，保留用于未来扩展）
/// - `db`: 数据库连接
/// - `redis`: Redis 连接池
/// - `s3_client`: OSS 存储客户端
/// - `token_cipher`: Token 加解密器
///
/// # 返回
/// 返回封装好的用户状态 `Arc<UserState>`
pub fn init_user(
    _cfg: &AppConfig,
    db: DatabaseConnection,
    redis: Pool,
    s3_client: Arc<oss::S3Client>,
    token_cipher: Arc<TokenCipher>,
) -> Arc<UserState> {
    Arc::new(
        UserState::new(
            db,
            redis,
            s3_client,
            token_cipher
        )
    )
}

/// 挂载用户模块的受保护路由
///
/// 将用户相关的路由挂载到 `/user` 路径下。
///
/// # 参数
/// - `router`: 已有的路由
/// - `state`: 用户模块状态
///
/// # 返回
/// 返回挂载了用户路由的新路由
pub fn mount_protected(
    router: Router,
    state: Arc<UserState>
) -> Router {
    router.nest("/user", UserController::routes().with_state(state))
}
