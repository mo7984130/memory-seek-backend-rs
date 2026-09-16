use types::{
    auth::user::UserId,
    media::media::{MediaId, MediaRecord},
};

step_derive::declare_pipeline!(
    MediaDeleteContext,
    MEDIA_DELETE_STEPS,
    MEDIA_DELETE_PIPELINE
);

/// 媒体删除步骤共享上下文，由服务查询并鉴权后在单个事务管道内消费。
pub struct MediaDeleteContext {
    pub user_id: UserId,
    pub medias: Vec<MediaRecord>,
}

impl MediaDeleteContext {
    /// 返回当前删除管道中的媒体 ID.
    pub fn media_ids(&self) -> Vec<MediaId> {
        self.medias.iter().map(|media| media.id).collect()
    }
}

pub async fn run_media_delete_pipeline(
    db: &sea_orm::DatabaseConnection,
    ctx: &mut MediaDeleteContext,
) -> common::Result<()> {
    MEDIA_DELETE_PIPELINE.run(db, ctx).await
}
