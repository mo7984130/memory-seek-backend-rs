use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

use super::test_config;

/// 测试清理守卫
///
/// 提供手动清理方法，测试结束时调用 `cleanup()` 清理测试数据。
pub struct CleanupGuard {
    db: DatabaseConnection,
    user_ids: Vec<i64>,
}

impl CleanupGuard {
    /// 创建新的清理守卫
    pub async fn new() -> Self {
        let cfg = test_config();
        let db = Database::connect(&cfg.database.url)
            .await
            .expect("连接测试数据库失败");

        Self {
            db,
            user_ids: Vec::new(),
        }
    }

    /// 注册需要清理的用户 ID
    pub fn track_user(&mut self, user_id: &str) {
        if let Ok(id) = user_id.parse::<i64>() {
            self.user_ids.push(id);
        }
    }

    /// 手动清理测试数据
    pub async fn cleanup(&self) {
        if self.user_ids.is_empty() {
            return;
        }

        let ids_str = self
            .user_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!("DELETE FROM auth_user WHERE id IN ({})", ids_str),
            ))
            .await;
    }
}
