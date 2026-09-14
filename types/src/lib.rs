pub mod audit;
pub mod auth;
pub mod backup;
pub mod cursor;
pub mod error;
pub mod macros;
pub mod photo;
pub mod user;
pub mod validators;

#[cfg(feature = "orm")]
pub mod db_init;

/// 回归: 实体中"插入时会省略"的 NOT NULL 列都必须带 DB 默认值。
///
/// 背景: 手写 INSERT / ActiveModel 漏填这些列时会触发 not-null 约束
/// (见 `photo_collection_photo.created_at` 与 `photo_comment.like_count` 的修复)。
/// schema sync 只增不改, 已存在的表需用 `docs/sql/` 下的脚本对齐。
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
        assert!(has_default(crate::photo::photo::Column::CreatedAt));
        assert!(has_default(crate::photo::collection::Column::CreatedAt));
        assert!(has_default(
            crate::photo::collection_photo::Column::CreatedAt
        ));
        assert!(has_default(crate::photo::comment::Column::CreatedAt));
        assert!(has_default(crate::photo::comment_like::Column::CreatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(crate::photo::face::Column::CreatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(crate::photo::person::Column::CreatedAt));
        assert!(has_default(crate::photo::photo_like::Column::CreatedAt));
        assert!(has_default(crate::photo::timeline_stat::Column::CreatedAt));
    }

    #[test]
    fn updated_at_columns_have_default() {
        assert!(has_default(crate::auth::user::Column::UpdatedAt));
        assert!(has_default(crate::photo::photo::Column::UpdatedAt));
        assert!(has_default(crate::photo::collection::Column::UpdatedAt));
        assert!(has_default(crate::photo::comment::Column::UpdatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(crate::photo::face::Column::UpdatedAt));
        #[cfg(feature = "face-engine")]
        assert!(has_default(crate::photo::person::Column::UpdatedAt));
        assert!(has_default(crate::photo::timeline_stat::Column::UpdatedAt));
    }

    #[test]
    fn comment_like_count_has_default() {
        assert!(has_default(crate::photo::comment::Column::LikeCount));
    }

    /// schema sync 用同一路径生成 CREATE TABLE, 因此直接断言 DDL 里的默认值。
    #[test]
    fn create_table_ddl_includes_timestamp_defaults() {
        use sea_orm::sea_query::PostgresQueryBuilder;
        use sea_orm::{DbBackend, Schema};

        // photo_photo 同时有 created_at / updated_at, 应各带一个默认值。
        let sql = Schema::new(DbBackend::Postgres)
            .create_table_from_entity(crate::photo::photo::Entity)
            .to_string(PostgresQueryBuilder);
        assert_eq!(
            sql.matches("DEFAULT CURRENT_TIMESTAMP").count(),
            2,
            "photo_photo 的 created_at/updated_at 应各带一个默认值: {sql}"
        );
    }
}
