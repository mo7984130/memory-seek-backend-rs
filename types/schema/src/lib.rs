//! 数据库 schema 编排:表结构同步与幂等索引初始化。
//!
//! 位于全部实体 crate **之上**(与 [`types_db_api`] 的依赖方向相反):
//! 表结构同步由各实体 crate 自我登记的前缀驱动 —— sea-orm 的实体注册表按
//! `module_path!()` 字符串前缀匹配,且 `sync` 只增不删,因此按前缀逐个同步是安全的。
//!
//! 依赖 `types-audit` / `types-identity` / `types-visual` 是为了把它们的实体
//! 纳入依赖图(实体通过 `inventory` 自我注册,需要被链接)。

use common::{ContextualError, ContextualResult};
use sea_orm::DatabaseConnection;

pub use types_db_api::{INIT_INDEXES, InitIndexFn, InitIndexFuture, SCHEMA_PREFIXES};

/// 初始化数据库:同步表结构 + 执行幂等索引初始化回调。
///
/// 幂等:重复调用不会重建已存在的表(sync 只增不删),索引回调亦为 `if_not_exists`。
pub async fn init_db(db: &DatabaseConnection) -> ContextualResult<()> {
    // 初始化表结构
    let mut synced = false;
    for prefix in SCHEMA_PREFIXES {
        synced = true;
        db.get_schema_registry(prefix.0)
            .sync(db)
            .await
            .map_err(|source| {
                ContextualError::error(
                    "db_sync_err",
                    "数据库同步失败",
                    source,
                    common::error::AppError::InternalServerError,
                )
            })?;
    }
    if !synced {
        // 没有实体 crate 登记前缀说明链接图里没有实体(或忘记登记), 表结构不会被同步。
        tracing::warn!("没有实体 crate 登记 schema 前缀, 表结构不会被同步");
    }

    // 幂等初始化索引
    for init_index in INIT_INDEXES {
        init_index(db).await?;
    }

    Ok(())
}

/// 回归: 实体中"插入时会省略"的 NOT NULL 列都必须带 DB 默认值。
///
/// 背景: 手写 INSERT / ActiveModel 漏填这些列时会触发 not-null 约束
/// (见 `visual_collection_visual.created_at` 与 `visual_comment.like_count` 的修复)。
/// schema sync 只增不改, 已存在的表需手工 `ALTER` 对齐(见 `docs/service-conventions.md` 的 schema 小节)。
#[cfg(test)]
mod column_default_tests {
    use sea_orm::ColumnTrait;

    fn has_default<C: ColumnTrait>(column: C) -> bool {
        column.def().get_column_default().is_some()
    }

    #[test]
    fn created_at_columns_have_default() {
        assert!(has_default(types_identity::auth::user::Column::CreatedAt));
        assert!(has_default(types_audit::Column::CreatedAt));
        assert!(has_default(types_visual::visual::Column::CreatedAt));
        assert!(has_default(types_visual::collection::Column::CreatedAt));
        assert!(has_default(
            types_visual::collection_visual::Column::CreatedAt
        ));
        assert!(has_default(types_visual::comment::Column::CreatedAt));
        assert!(has_default(types_visual::comment_like::Column::CreatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(types_visual::face::Column::CreatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(types_visual::person::Column::CreatedAt));
        assert!(has_default(types_visual::visual_like::Column::CreatedAt));
        assert!(has_default(types_visual::timeline_stat::Column::CreatedAt));
    }

    #[test]
    fn updated_at_columns_have_default() {
        assert!(has_default(types_identity::auth::user::Column::UpdatedAt));
        assert!(has_default(types_visual::visual::Column::UpdatedAt));
        assert!(has_default(types_visual::collection::Column::UpdatedAt));
        assert!(has_default(types_visual::comment::Column::UpdatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(types_visual::face::Column::UpdatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(types_visual::person::Column::UpdatedAt));
        assert!(has_default(types_visual::timeline_stat::Column::UpdatedAt));
    }

    #[test]
    fn comment_like_count_has_default() {
        assert!(has_default(types_visual::comment::Column::LikeCount));
    }

    /// schema sync 用同一路径生成 CREATE TABLE, 因此直接断言 DDL 里的默认值。
    #[test]
    fn create_table_ddl_includes_timestamp_defaults() {
        use sea_orm::sea_query::PostgresQueryBuilder;
        use sea_orm::{DbBackend, Schema};

        // visual_visual 同时有 created_at / updated_at, 应各带一个默认值。
        let sql = Schema::new(DbBackend::Postgres)
            .create_table_from_entity(types_visual::visual::Entity)
            .to_string(PostgresQueryBuilder);
        assert_eq!(
            sql.matches("DEFAULT CURRENT_TIMESTAMP").count(),
            2,
            "visual_visual 的 created_at/updated_at 应各带一个默认值: {sql}"
        );
    }
}
