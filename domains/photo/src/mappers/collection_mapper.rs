pub(crate) struct CollectionMapper;

use std::collections::HashMap;

use common::ext::{ContextOptionExt, OkExt};
use common::time::now;
use common::{
    DbConn as ConnectionTrait,
    error::{AppError, ContextualError, contextual::Result},
};
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbBackend, EntityName, EntityTrait, Iden, IdenStatic,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Statement,
};
use types::photo::collection::{self, CollectionId};
use types::photo::collection_photo;
use types::photo::photo::PhotoId;
use types::{auth::user::UserId, photo::collection::*};

// 创建
impl CollectionMapper {
    // 添加收藏夹照片
    // 会同时修改collection 和 collection_photo 两个表
    // 返回插入后的照片总数
    pub async fn add_photos_batch(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
        photo_ids: Vec<PhotoId>,
    ) -> Result<u64> {
        let collection_photo_table_name = collection_photo::Entity.table_name();
        let collection_table_name = collection::Entity.table_name();

        let cp_collection_id = collection_photo::Column::CollectionId.to_string();
        let cp_photo_id = collection_photo::Column::PhotoId.to_string();
        let cp_user_id = collection_photo::Column::UserId.to_string();
        let c_id = collection::Column::Id.to_string();
        let c_photo_count = collection::Column::PhotoCount.to_string();

        let sql = format!(
            r#"
                WITH ins AS (
                    INSERT INTO "{collection_photo_table_name}" ("{cp_collection_id}", "{cp_photo_id}", "{cp_user_id}")
                    SELECT $1, unnest($2::bigint[]), $3
                    ON CONFLICT ("{cp_collection_id}", "{cp_photo_id}") DO NOTHING
                    RETURNING 1
                )
                UPDATE "{collection_table_name}"
                SET "{c_photo_count}" = "{c_photo_count}" + (SELECT count(*) FROM ins)
                WHERE "{c_id}" = $1
                RETURNING "{c_photo_count}"
                "#
        );

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            &sql,
            [collection_id.into(), photo_ids.into(), user_id.into()],
        );

        let result = db.query_one(stmt).await?.context_warn_none(
            "collection_not_found",
            "收藏夹不存在",
            AppError::not_found("收藏夹不存在"),
        )?;

        let new_count: i64 = result.try_get("", &c_photo_count)?;

        u64::try_from(new_count).map_err(|error| {
            ContextualError::error(
                "collection_photo_count_negative",
                "photo_count 异常为负值",
                error,
                AppError::InternalServerError,
            )
        })
    }

    /// 创建收藏夹.
    pub async fn insert(
        db: &impl ConnectionTrait,
        user_id: UserId,
        name: String,
        description: Option<String>,
    ) -> Result<CollectionRecord> {
        let now = now();
        ActiveModel {
            user_id: Set(user_id),
            name: Set(name),
            description: Set(description),
            photo_count: Set(0),
            cover_file_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .map(CollectionRecord::from)?
        .to_ok()
    }
}

// 修改
impl CollectionMapper {
    /// 更新收藏夹信息
    pub async fn update_cover_photo(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        cover_photo_id: PhotoId,
        cover_file_id: String,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::CoverPhotoId, Expr::value(cover_photo_id))
            .col_expr(Column::CoverFileId, Expr::value(cover_file_id))
            .col_expr(Column::UpdatedAt, Expr::value(now()))
            .filter(Column::Id.eq(collection_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// 批量更新多个收藏夹的照片计数.
    pub async fn update_photo_count_delta_batch(
        db: &impl ConnectionTrait,
        deltas: &HashMap<CollectionId, i64>,
    ) -> Result<()> {
        let (ids, counts): (Vec<i64>, Vec<i64>) = deltas
            .iter()
            .map(|(id, count)| (i64::from(*id), *count))
            .unzip();

        let table = Entity.table_name();
        let col_id = Column::Id.as_str();
        let col_photo_count = Column::PhotoCount.as_str();

        let stmt = Statement::from_sql_and_values(
            db.get_database_backend(),
            format!(
                r#"
                    UPDATE {table} c
                    SET {col_photo_count} = c.{col_photo_count} + (delta.cnt)
                    FROM UNNEST($1::bigint[], $2::bigint[]) AS delta(id, cnt)
                    WHERE c.{col_id} = delta.id
                    "#,
            ),
            [ids.into(), counts.into()],
        );

        db.execute(stmt).await?;

        Ok(())
    }

    /// 增量更新照片计数.
    pub async fn update_photo_count_delta(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        delta: i64,
    ) -> Result<()> {
        Entity::update_many()
            .col_expr(Column::PhotoCount, Expr::col(Column::PhotoCount).add(delta))
            .filter(Column::Id.eq(collection_id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// 更新收藏夹信息.
    pub async fn update_info(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        user_id: UserId,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<u64> {
        if name.is_none() && description.is_none() {
            return Ok(0);
        }
        let mut update = Entity::update_many();
        if let Some(name) = name {
            update = update.col_expr(Column::Name, Expr::value(name));
        }
        if let Some(description) = description {
            update = update.col_expr(Column::Description, Expr::value(description));
        }
        let result = update
            .col_expr(Column::UpdatedAt, Expr::value(now()))
            .filter(Column::Id.eq(collection_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}

// 查询
impl CollectionMapper {
    /// 批量获取id 和 name
    pub async fn query_id_and_name_by_ids(
        db: &impl ConnectionTrait,
        ids: &[CollectionId],
    ) -> Result<Vec<(CollectionId, String)>> {
        Entity::find()
            .filter(Column::Id.is_in(ids.iter().copied()))
            .select_only()
            .column(Column::Id)
            .column(Column::Name)
            .into_tuple::<(CollectionId, String)>()
            .all(db)
            .await?
            .to_ok()
    }

    /// 通过user_id查询全部收藏夹.
    pub async fn query_by_user_id(
        db: &impl ConnectionTrait,
        user_id: UserId,
    ) -> Result<Vec<CollectionRecord>> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await?
            .into_iter()
            .map(CollectionRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }

    /// 检查收藏夹是否属于对应用户.
    pub async fn is_belong(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<bool> {
        let count = Entity::find()
            .filter(Column::Id.eq(collection_id))
            .filter(Column::UserId.eq(user_id))
            .count(db)
            .await?;

        Ok(count > 0)
    }

    /// 确保收藏夹属于对应用户
    pub async fn ensure_belong(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<()> {
        if !Self::is_belong(db, user_id, collection_id).await? {
            return Err(ContextualError::warn_without_source(
                "collection_not_belong_user",
                "收藏夹不属于用户",
                AppError::forbidden("该收藏夹不属于你"),
            ));
        }
        Ok(())
    }

    /// 确保收藏夹属于对应用户, 并且返回收藏夹记录.
    pub async fn ensure_belong_with_return(
        db: &impl ConnectionTrait,
        user_id: UserId,
        collection_id: CollectionId,
    ) -> Result<CollectionRecord> {
        Entity::find()
            .filter(Column::Id.eq(collection_id))
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await?
            .context_warn_none(
                "collection_not_belong_user",
                "收藏夹不属于用户",
                AppError::forbidden("该收藏夹不属于你"),
            )
            .map(CollectionRecord::from)
    }
}

// 删除
impl CollectionMapper {
    /// 删除收藏夹.
    pub async fn delete_by_id(
        db: &impl ConnectionTrait,
        collection_id: CollectionId,
        user_id: UserId,
    ) -> Result<u64> {
        Entity::delete_many()
            .filter(Column::Id.eq(collection_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?
            .rows_affected
            .to_ok()
    }
}
