use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

use super::test_config;

/// 测试清理守卫
///
/// - 创建时可指定需要预先清理的用户名（防上次残留）
/// - 测试中通过 `track_user()` 追踪新创建的用户 ID
/// - 测试结束时调用 `cleanup()` 或 Drop 自动清理
#[allow(dead_code)]
pub struct CleanupGuard {
    db: DatabaseConnection,
    user_ids: Vec<i64>,
    user_names: Vec<String>,
}

#[allow(dead_code)]
impl CleanupGuard {
    /// 创建新的清理守卫（不预先清理）
    pub async fn new() -> Self {
        let cfg = test_config();
        let db = Database::connect(&cfg.database.url)
            .await
            .expect("连接测试数据库失败");

        Self {
            db,
            user_ids: Vec::new(),
            user_names: Vec::new(),
        }
    }

    /// 创建清理守卫并预先清理指定用户名的残留数据
    pub async fn new_with_cleanup(usernames: &[&str]) -> Self {
        let mut guard = Self::new().await;

        // 先查询这些用户名对应的 user_id，用于级联清理 photo 数据
        for &name in usernames {
            guard.user_names.push(name.to_string());

            // 查询已有 user_id
            if let Ok(Some(row)) = guard
                .db
                .query_one(Statement::from_string(
                    sea_orm::DatabaseBackend::Postgres,
                    format!(
                        "SELECT id FROM auth_user WHERE username = '{}'",
                        name.replace('\'', "''")
                    ),
                ))
                .await
            {
                if let Ok(id) = row.try_get::<i64>("", "id") {
                    guard.delete_cascade(&[id]).await;
                }
            }

            let _ = guard
                .db
                .execute(Statement::from_string(
                    sea_orm::DatabaseBackend::Postgres,
                    format!("DELETE FROM auth_user WHERE username = '{}'", name),
                ))
                .await;
        }
        guard
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

        self.delete_cascade(&self.user_ids).await;

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

    /// 级联删除用户关联的 photo 数据
    async fn delete_cascade(&self, user_ids: &[i64]) {
        if user_ids.is_empty() {
            return;
        }

        let ids_str = user_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");

        // 按依赖顺序删除 photo 关联表
        let tables_with_user_id = [
            "photo_photo_like",
            "photo_comment_like",
            "photo_comment",
            "photo_collection_photo",
            "photo_collection",
            "photo_photo",
        ];

        for table in tables_with_user_id {
            let _ = self
                .db
                .execute(Statement::from_string(
                    sea_orm::DatabaseBackend::Postgres,
                    format!("DELETE FROM {} WHERE user_id IN ({})", table, ids_str),
                ))
                .await;
        }

        // photo_face_feature 通过 photo_id 子查询删除
        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "DELETE FROM photo_face_feature WHERE photo_id IN \
                     (SELECT id FROM photo_photo WHERE user_id IN ({}))",
                    ids_str
                ),
            ))
            .await;

        // photo_timeline_stat 是聚合表，测试中直接清空
        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "TRUNCATE TABLE photo_timeline_stat".to_string(),
            ))
            .await;
    }
}
