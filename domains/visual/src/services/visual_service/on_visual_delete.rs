use types_identity::auth::user::UserId;
use types_visual::{visual::VisualId, visual::VisualRecord};

step_derive::declare_pipeline!(
    VisualDeleteContext,
    MEDIA_DELETE_STEPS,
    MEDIA_DELETE_PIPELINE
);

/// 影像删除步骤共享上下文，由服务查询并鉴权后在单个事务管道内消费。
pub struct VisualDeleteContext {
    pub user_id: UserId,
    pub visuals: Vec<VisualRecord>,
}

impl VisualDeleteContext {
    /// 返回当前删除管道中的影像 ID.
    pub fn visual_ids(&self) -> Vec<VisualId> {
        self.visuals.iter().map(|visual| visual.id).collect()
    }
}

pub async fn run_visual_delete_pipeline(
    db: &sea_orm::DatabaseConnection,
    ctx: &mut VisualDeleteContext,
) -> common_core::Result<()> {
    MEDIA_DELETE_PIPELINE.run(db, ctx).await
}
