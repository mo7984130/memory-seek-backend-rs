use common_core::DbConn as ConnectionTrait;
use common_core::error::contextual::Result;
use common_core::time::now;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use types_visual::derivative::{
    ActiveModel, Column, DerivativeKind, DerivativeRecord, DerivativeStatus, Entity,
    NewDerivativeRecord, VisualDerivativeId,
};
use types_visual::visual::VisualId;

pub struct DerivativeMapper;

impl DerivativeMapper {
    /// 幂等插入待生成的衍生片记录(已存在则跳过).
    ///
    /// `(visual_id, kind)` 上有唯一索引, 重复上传事件不会产生多条记录。
    pub async fn insert_pending(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
        kinds: &[DerivativeKind],
    ) -> Result<()> {
        if kinds.is_empty() {
            return Ok(());
        }
        let records = kinds
            .iter()
            .map(|&kind| NewDerivativeRecord { visual_id, kind });
        Entity::insert_many(records.map(ActiveModel::from))
            .on_conflict(
                OnConflict::columns([Column::VisualId, Column::Kind])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
        Ok(())
    }

    /// 标记为生成中.
    pub async fn mark_running(db: &impl ConnectionTrait, id: VisualDerivativeId) -> Result<()> {
        Self::update_status(db, id, DerivativeStatus::Running, None, None).await
    }

    /// 标记为已就绪, 并记录对象 key.
    pub async fn mark_ready(
        db: &impl ConnectionTrait,
        id: VisualDerivativeId,
        object_key: &str,
    ) -> Result<()> {
        Self::update_status(
            db,
            id,
            DerivativeStatus::Ready,
            Some(object_key.to_string()),
            None,
        )
        .await
    }

    /// 标记为生成失败, 并记录原因.
    pub async fn mark_failed(
        db: &impl ConnectionTrait,
        id: VisualDerivativeId,
        error: &str,
    ) -> Result<()> {
        Self::update_status(
            db,
            id,
            DerivativeStatus::Failed,
            None,
            Some(error.to_string()),
        )
        .await
    }

    /// 标记为待生成(用于启动恢复将卡住的 running 重置).
    pub async fn mark_pending(db: &impl ConnectionTrait, id: VisualDerivativeId) -> Result<()> {
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

    /// 查询需要重新入队的记录(待生成 / 卡在生成中的, 用于启动恢复).
    pub async fn query_recoverable(db: &impl ConnectionTrait) -> Result<Vec<DerivativeRecord>> {
        let models = Entity::find()
            .filter(Column::Status.is_in([DerivativeStatus::Pending, DerivativeStatus::Running]))
            .all(db)
            .await?;
        Ok(models.into_iter().map(DerivativeRecord::from).collect())
    }

    async fn update_status(
        db: &impl ConnectionTrait,
        id: VisualDerivativeId,
        status: DerivativeStatus,
        object_key: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        let mut active = ActiveModel {
            id: Set(id),
            status: Set(status),
            updated_at: Set(now()),
            ..Default::default()
        };
        if let Some(object_key) = object_key {
            active.object_key = Set(Some(object_key));
        }
        if let Some(error) = error {
            active.error = Set(Some(error));
        }
        active.update(db).await?;
        Ok(())
    }
}
