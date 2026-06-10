pub(crate) struct CollectionMapper;

use std::collections::HashMap;

use chrono::Utc;
use common::error::AppError;
use common::ext::{OkExt, ToErr, log_warn};
use common::{Result, ext::ResultErrExt};
use entities::photo::collection::CollectionId;
use entities::{auth::user::UserId, photo::collection::*};
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityName, EntityTrait, IdenStatic,
    QueryFilter, QueryOrder, QuerySelect, Statement,
};

impl CollectionMapper {
    pub async fn query_favorite_collection_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
    ) -> Result<Option<CollectionId>> {
        Entity::find()
            .filter(Column::UserId.eq(user_id.0))
            .filter(Column::IsFavorite.eq(true))
            .select_only()
            .column(Column::Id)
            .into_tuple::<i64>()
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")?
            .map(CollectionId)
            .to_ok()
    }

    pub async fn query_by_user_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
    ) -> Result<Vec<Model>> {
        Entity::find()
            .filter(Column::UserId.eq(user_id.0))
            .order_by_asc(Column::IsFavorite)
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
            .trace_internal_err("db_query_err", "查询收藏夹失败")
    }

    pub async fn query_favorite_by_user_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
    ) -> Result<Option<Model>> {
        Entity::find()
            .filter(Column::UserId.eq(user_id.0))
            .filter(Column::IsFavorite.eq(true))
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询失败")
    }

    pub async fn insert(
        db: &impl ConnectionTrait,
        user_id: UserId,
        name: String,
        description: Option<String>,
        is_favorite: bool,
    ) -> Result<Model> {
        let now = Utc::now();
        ActiveModel {
            user_id: Set(user_id.0),
            name: Set(name),
            description: Set(description),
            photo_count: Set(0),
            cover_file_id: Set(None),
            is_favorite: Set(is_favorite),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .trace_internal_err("db_insert_err", "创建收藏夹失败")
    }

    pub async fn update_photo_count(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        count: u64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::PhotoCount, Expr::value(count))
            .filter(Column::Id.eq(collection_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_update_err", "更新失败")?;

        Ok(())
    }

    pub async fn decr_photo_count_batch(
        db: &impl ConnectionTrait,
        decrements: &HashMap<CollectionId, u64>,
    ) -> Result<()> {
        if decrements.is_empty() {
            return Ok(());
        }

        let (ids, counts): (Vec<i64>, Vec<i64>) = decrements
            .iter()
            .map(|(id, count)| (id.0, *count as i64))
            .unzip();

        let table = Entity.table_name();
        let col_id = Column::Id.as_str();
        let col_photo_count = Column::PhotoCount.as_str();

        let stmt = Statement::from_sql_and_values(
            db.get_database_backend(),
            format!(
                r#"
                    UPDATE {table} c
                    SET {col_photo_count} = c.{col_photo_count} - delta.cnt
                    FROM UNNEST($1::bigint[], $2::bigint[]) AS delta(id, cnt)
                    WHERE c.{col_id} = delta.id
                    "#,
            ),
            [ids.into(), counts.into()],
        );

        db.execute(stmt)
            .await
            .trace_internal_err("db_update_err", "批量更新收藏夹照片数失败")?;

        Ok(())
    }

    pub async fn update_info(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        user_id: UserId,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<()> {
        // 如果两个字段都为 None，直接返回，无需操作
        if name.is_none() && description.is_none() {
            return Ok(());
        }

        let mut update = Entity::update_many();

        if let Some(name) = name {
            update = update.col_expr(Column::Name, Expr::value(name));
        }

        if let Some(description) = description {
            update = update.col_expr(Column::Description, Expr::value(description));
        }

        let result = update
            .col_expr(Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(Column::Id.eq(collection_id.0))
            .filter(Column::UserId.eq(user_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_update_err", "修改收藏夹信息失败")?;

        if result.rows_affected == 0 {
            return log_warn(
                "update_rows_affected",
                "修改的影响行为零",
                "",
                AppError::bad_request("修改收藏夹信息失败"),
            )
            .to_err();
        }

        Ok(())
    }

    pub async fn delete_by_id(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        user_id: UserId,
    ) -> Result<()> {
        let result = Entity::delete_many()
            .filter(Column::Id.eq(collection_id.0))
            .filter(Column::UserId.eq(user_id.0))
            .exec(db)
            .await
            .trace_internal_err("db_delete_err", "删除收藏夹失败")?;

        if result.rows_affected == 0 {
            return log_warn(
                "delete_rows_affected",
                "删除的影响行为零",
                "",
                AppError::bad_request("删除收藏夹失败"),
            )
            .to_err();
        }

        Ok(())
    }
}
