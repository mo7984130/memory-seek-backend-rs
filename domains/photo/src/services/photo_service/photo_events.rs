#[cfg(feature = "face")]
use bytes::Bytes;
#[cfg(feature = "face")]
use types::photo::person::PersonId;
use types::photo::photo::PhotoRecord;

/// 照片主记录落库后发布的事件，供时间线、人脸等后续服务消费。
pub struct AfterPhotoUpload {
    pub photo: PhotoRecord,
    /// 保留原始字节，供启用 `face` 后的人脸识别订阅者消费。
    #[cfg(feature = "face")]
    pub file_data: Bytes,
}

step_derive::declare_async_event!(
    crate::state::PhotoState,
    AfterPhotoUpload,
    AFTER_PHOTO_UPLOAD_CONSUMERS,
    publish_after_photo_upload,
    "after_photo_upload",
);

/// 照片及其文件删除后发布的事件，供缓存等后续服务消费。
pub struct AfterPhotoDelete {
    pub photos: Vec<PhotoRecord>,
    #[cfg(feature = "face")]
    pub affected_person_ids: Vec<PersonId>,
}

step_derive::declare_async_event!(
    crate::state::PhotoState,
    AfterPhotoDelete,
    AFTER_PHOTO_DELETE_CONSUMERS,
    publish_after_photo_delete,
    "after_photo_delete",
);
