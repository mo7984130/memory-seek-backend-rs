use common::Pool;
use email::EmailClient;
use sea_orm::DatabaseConnection;

/// 认证服务状态
pub struct AuthState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub email_client: EmailClient,
}

impl AuthState {
    /// 创建认证服务状态实例
    ///
    /// # 参数
    /// - `db`: PostgreSQL 数据库连接
    /// - `redis`: Redis 连接池
    /// - `email_client`: 邮件发送客户端
    pub fn new(db: DatabaseConnection, redis: Pool, email_client: EmailClient) -> Self {
        Self {
            db,
            redis,
            email_client,
        }
    }
}
