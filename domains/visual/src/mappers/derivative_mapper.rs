use common_core::DbConn as ConnectionTrait;
use common_core::error::contextual::Result;
use common_core::time::now;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_visual::derivative::{
    ActiveModel, Column, DerivativeKind, DerivativeRecord, DerivativeStatus, Entity,
    NewDerivativeRecord, VisualDerivativeId,
};
use types_visual::visual::VisualId;

pub struct DerivativeMapper;

impl DerivativeMapper {
    /// 幂等插入待生成的衍生片记录(已存在则跳过).
    ///
    /// 不使用 DB 唯一约束: 依赖先查后插保证幂等, 避免裸 SQL 唯一索引与
    /// sea-orm schema-sync 在重启时冲突。
    pub async fn insert_pending(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
        kinds: &[DerivativeKind],
    ) -> Result<()> {
        if kinds.is_empty() {
            return Ok(());
        }
        let existing_kinds: Vec<DerivativeKind> = Entity::find()
            .filter(Column::VisualId.eq(visual_id))
            .all(db)
            .await?
            .into_iter()
            .map(|model| model.kind)
            .collect();
        let pending: Vec<ActiveModel> = kinds
            .iter()
            .copied()
            .filter(|kind| !existing_kinds.contains(kind))
            .map(|kind| NewDerivativeRecord { visual_id, kind }.into())
            .collect();
        if pending.is_empty() {
            return Ok(());
        }
        Entity::insert_many(pending)
            .exec_without_returning(db)
            .await?;
        Ok(())
    }

    /// 标记为生成中; 返回是否命中记录(影像被删后记录不存在时为 `false`)。
    pub async fn mark_running(db: &impl ConnectionTrait, id: VisualDerivativeId) -> Result<bool> {
        Self::update_status(db, id, DerivativeStatus::Running, None, None).await
    }

    /// 标记为已就绪并记录对象 key; 返回是否命中记录。
    pub async fn mark_ready(
        db: &impl ConnectionTrait,
        id: VisualDerivativeId,
        object_key: &str,
    ) -> Result<bool> {
        Self::update_status(
            db,
            id,
            DerivativeStatus::Ready,
            Some(object_key.to_string()),
            None,
        )
        .await
    }

    /// 标记为生成失败并记录原因; 返回是否命中记录。
    pub async fn mark_failed(
        db: &impl ConnectionTrait,
        id: VisualDerivativeId,
        error: &str,
    ) -> Result<bool> {
        Self::update_status(
            db,
            id,
            DerivativeStatus::Failed,
            None,
            Some(error.to_string()),
        )
        .await
    }

    /// 重置为待生成(启动恢复将卡住的 running 复位); 返回是否命中记录。
    pub async fn mark_pending(db: &impl ConnectionTrait, id: VisualDerivativeId) -> Result<bool> {
        Self::update_status(db, id, DerivativeStatus::Pending, None, None).await
    }

    /// 查询某张影像的全部衍生片记录.
    pub async fn query_by_visual_id(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
    ) -> Result<Vec<DerivativeRecord>> {
        let models = Entity::find()
            .filter(Column::VisualId.eq(visual_id))
            .all(db)
            .await?;
        Ok(models.into_iter().map(DerivativeRecord::from).collect())
    }

    /// 查询某张影像指定类型的衍生片记录.
    pub async fn query_one(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
        kind: DerivativeKind,
    ) -> Result<Option<DerivativeRecord>> {
        let model = Entity::find()
            .filter(Column::VisualId.eq(visual_id))
            .filter(Column::Kind.eq(kind))
            .one(db)
            .await?;
        Ok(model.map(DerivativeRecord::from))
    }

    /// 删除若干影像的全部衍生片记录(随影像删除).
    pub async fn delete_by_visual_ids(
        db: &impl ConnectionTrait,
        visual_ids: &[VisualId],
    ) -> Result<()> {
        Entity::delete_many()
            .filter(Column::VisualId.is_in(visual_ids.iter().copied()))
            .exec(db)
            .await?;
        Ok(())
    }

    /// 查询需要重新入队的记录(待生成 / 卡在生成中的, 用于启动恢复).
    pub async fn query_recoverable(db: &impl ConnectionTrait) -> Result<Vec<DerivativeRecord>> {
        let models = Entity::find()
            .filter(Column::Status.is_in([DerivativeStatus::Pending, DerivativeStatus::Running]))
            .all(db)
            .await?;
        Ok(models.into_iter().map(DerivativeRecord::from).collect())
    }

    /// 按主键更新状态, 返回是否命中记录.
    ///
    /// 用 `update_many` 而非 `ActiveModel::update`: 目标记录可能已被删除
    /// (影像删除时连带清理), 0 行受影响应视为"记录已不存在"而非错误。
    async fn update_status(
        db: &impl ConnectionTrait,
        id: VisualDerivativeId,
        status: DerivativeStatus,
        object_key: Option<String>,
        error: Option<String>,
    ) -> Result<bool> {
        let mut update = Entity::update_many()
            .filter(Column::Id.eq(id))
            .col_expr(Column::Status, Expr::value(status))
            .col_expr(Column::UpdatedAt, Expr::value(now()));
        if let Some(object_key) = object_key {
            update = update.col_expr(Column::ObjectKey, Expr::value(object_key));
        }
        if let Some(error) = error {
            update = update.col_expr(Column::Error, Expr::value(error));
        }
        let result = update.exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}
