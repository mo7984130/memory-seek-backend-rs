pub mod audit;
pub mod auth;
pub mod backup;
pub mod cursor;
pub mod error;
pub mod macros;
pub mod user;
pub mod validators;
pub mod visual;

#[cfg(feature = "orm")]
pub mod db_init;

/// 回归: 实体中"插入时会省略"的 NOT NULL 列都必须带 DB 默认值。
///
/// 背景: 手写 INSERT / ActiveModel 漏填这些列时会触发 not-null 约束
/// (见 `visual_collection_visual.created_at` 与 `visual_comment.like_count` 的修复)。
/// schema sync 只增不改, 已存在的表需手工 `ALTER` 对齐(见 `docs/service-conventions.md` 的 schema 小节)。
#[cfg(all(test, feature = "orm"))]
mod column_default_tests {
    use sea_orm::ColumnTrait;

    fn has_default<C: ColumnTrait>(column: C) -> bool {
        column.def().get_column_default().is_some()
    }

    #[test]
    fn created_at_columns_have_default() {
        assert!(has_default(crate::auth::user::Column::CreatedAt));
        assert!(has_default(crate::audit::Column::CreatedAt));
        assert!(has_default(crate::visual::visual::Column::CreatedAt));
        assert!(has_default(crate::visual::collection::Column::CreatedAt));
        assert!(has_default(
            crate::visual::collection_visual::Column::CreatedAt
        ));
        assert!(has_default(crate::visual::comment::Column::CreatedAt));
        assert!(has_default(crate::visual::comment_like::Column::CreatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(crate::visual::face::Column::CreatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(crate::visual::person::Column::CreatedAt));
        assert!(has_default(crate::visual::visual_like::Column::CreatedAt));
        assert!(has_default(crate::visual::timeline_stat::Column::CreatedAt));
    }

    #[test]
    fn updated_at_columns_have_default() {
        assert!(has_default(crate::auth::user::Column::UpdatedAt));
        assert!(has_default(crate::visual::visual::Column::UpdatedAt));
        assert!(has_default(crate::visual::collection::Column::UpdatedAt));
        assert!(has_default(crate::visual::comment::Column::UpdatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(crate::visual::face::Column::UpdatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(crate::visual::person::Column::UpdatedAt));
        assert!(has_default(crate::visual::timeline_stat::Column::UpdatedAt));
    }

    #[test]
    fn comment_like_count_has_default() {
        assert!(has_default(crate::visual::comment::Column::LikeCount));
    }

    /// schema sync 用同一路径生成 CREATE TABLE, 因此直接断言 DDL 里的默认值。
    #[test]
    fn create_table_ddl_includes_timestamp_defaults() {
        use sea_orm::sea_query::PostgresQueryBuilder;
        use sea_orm::{DbBackend, Schema};

        // visual_visual 同时有 created_at / updated_at, 应各带一个默认值。
        let sql = Schema::new(DbBackend::Postgres)
            .create_table_from_entity(crate::visual::visual::Entity)
            .to_string(PostgresQueryBuilder);
        assert_eq!(
            sql.matches("DEFAULT CURRENT_TIMESTAMP").count(),
            2,
            "visual_visual 的 created_at/updated_at 应各带一个默认值: {sql}"
        );
    }
}
