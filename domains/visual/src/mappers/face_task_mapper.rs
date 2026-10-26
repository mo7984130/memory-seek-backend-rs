use common_core::DbConn as ConnectionTrait;
use common_core::error::contextual::Result;
use common_core::time::now;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use types_visual::face_task::{
    ActiveModel, Column, Entity, FaceTaskRecord, FaceTaskStatus, NewFaceTaskRecord,
};
use types_visual::visual::VisualId;

pub struct FaceTaskMapper;

impl FaceTaskMapper {
    /// 幂等插入待检测任务，已存在则跳过；依赖先查后插而非唯一索引保证幂等
    pub async fn insert_pending(db: &impl ConnectionTrait, visual_id: VisualId) -> Result<()> {
        let exists = Entity::find()
            .filter(Column::VisualId.eq(visual_id))
            .one(db)
            .await?
            .is_some();
        if exists {
            return Ok(());
        }
        let pending: ActiveModel = NewFaceTaskRecord { visual_id }.into();
        Entity::insert(pending).exec_without_returning(db).await?;
        Ok(())
    }

    /// 将待检测任务标记为检测中并返回是否命中；已完成/失败任务不会被重新消费
    pub async fn mark_running(db: &impl ConnectionTrait, visual_id: VisualId) -> Result<bool> {
        let result = Entity::update_many()
            .filter(Column::VisualId.eq(visual_id))
            .filter(Column::Status.eq(FaceTaskStatus::Pending))
            .col_expr(Column::Status, Expr::value(FaceTaskStatus::Running))
            .col_expr(Column::UpdatedAt, Expr::value(now()))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// 将检测中任务标记为完成并返回是否命中；与“人脸落库”同事务调用，未命中则跳过
    pub async fn mark_done(db: &impl ConnectionTrait, visual_id: VisualId) -> Result<bool> {
        let result = Entity::update_many()
            .filter(Column::VisualId.eq(visual_id))
            .filter(Column::Status.eq(FaceTaskStatus::Running))
            .col_expr(Column::Status, Expr::value(FaceTaskStatus::Done))
            .col_expr(Column::UpdatedAt, Expr::value(now()))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// 重置为待检测(失败重试与启动恢复将 running 复位); 返回是否命中
    pub async fn mark_pending(db: &impl ConnectionTrait, visual_id: VisualId) -> Result<bool> {
        Self::update_status(db, visual_id, FaceTaskStatus::Pending, None).await
    }

    /// 标记为检测失败并记录原因; 返回是否命中
    pub async fn mark_failed(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
        error: &str,
    ) -> Result<bool> {
        Self::update_status(
            db,
            visual_id,
            FaceTaskStatus::Failed,
            Some(error.to_string()),
        )
        .await
    }

    /// 递增尝试次数并返回新值; 任务不存在时返回 `None`
    pub async fn bump_attempt(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
    ) -> Result<Option<i32>> {
        let current: Option<i32> = Entity::find()
            .select_only()
            .column(Column::Attempts)
            .filter(Column::VisualId.eq(visual_id))
            .into_tuple::<i32>()
            .one(db)
            .await?;
        let Some(current) = current else {
            return Ok(None);
        };
        let next = current + 1;
        let rows = Entity::update_many()
            .filter(Column::VisualId.eq(visual_id))
            .col_expr(Column::Attempts, Expr::value(next))
            .col_expr(Column::UpdatedAt, Expr::value(now()))
            .exec(db)
            .await?
            .rows_affected;
        Ok((rows > 0).then_some(next))
    }

    /// 查询需要重新入队的任务(待检测 / 卡在检测中的, 用于启动恢复)
    pub async fn query_recoverable(db: &impl ConnectionTrait) -> Result<Vec<FaceTaskRecord>> {
        let models = Entity::find()
            .filter(Column::Status.is_in([FaceTaskStatus::Pending, FaceTaskStatus::Running]))
            .all(db)
            .await?;
        Ok(models.into_iter().map(FaceTaskRecord::from).collect())
    }

    /// 删除若干影像的检测任务(随影像删除)
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

    /// 按主键更新任务状态并返回是否命中；0 行受影响视为记录已不存在而非错误
    async fn update_status(
        db: &impl ConnectionTrait,
        visual_id: VisualId,
        status: FaceTaskStatus,
        error: Option<String>,
    ) -> Result<bool> {
        let mut update = Entity::update_many()
            .filter(Column::VisualId.eq(visual_id))
            .col_expr(Column::Status, Expr::value(status))
            .col_expr(Column::UpdatedAt, Expr::value(now()));
        if let Some(error) = error {
            update = update.col_expr(Column::Error, Expr::value(error));
        }
        let result = update.exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}
