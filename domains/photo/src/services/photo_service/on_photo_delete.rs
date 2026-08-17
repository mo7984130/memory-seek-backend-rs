#[cfg(feature = "face")]
use types::photo::person::PersonId;
use types::{
    auth::user::UserId,
    photo::photo::{PhotoId, PhotoRecord},
};

step_derive::declare_pipeline!(
    PhotoDeleteContext,
    PHOTO_DELETE_STEPS,
    PHOTO_DELETE_PIPELINE
);

/// 照片删除步骤共享上下文，由服务查询并鉴权后在单个事务管道内消费。
pub struct PhotoDeleteContext {
    pub user_id: UserId,
    pub photos: Vec<PhotoRecord>,
    #[cfg(feature = "face")]
    /// 删除人脸步骤更新过统计的人物 ID，供事务提交后失效缓存。
    pub affected_person_ids: Vec<PersonId>,
}

impl PhotoDeleteContext {
    /// 返回当前删除管道中的照片 ID.
    pub fn photo_ids(&self) -> Vec<PhotoId> {
        self.photos.iter().map(|photo| photo.id).collect()
    }
}

pub async fn run_photo_delete_pipeline(
    db: &sea_orm::DatabaseConnection,
    ctx: &mut PhotoDeleteContext,
) -> common::Result<()> {
    PHOTO_DELETE_PIPELINE.run(db, ctx).await
}
