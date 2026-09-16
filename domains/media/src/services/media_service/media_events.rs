#[cfg(feature = "face")]
use bytes::Bytes;
use types::media::media::MediaRecord;

/// 照片主记录落库后发布的事件，供时间线、人脸等后续服务消费。
pub struct AfterMediaUpload {
    pub media: MediaRecord,
    /// 保留原始字节，供启用 `face` 后的人脸识别订阅者消费。
    #[cfg(feature = "face")]
    pub file_data: Bytes,
}

step_derive::declare_async_event!(
    crate::state::MediaState,
    AfterMediaUpload,
    AFTER_MEDIA_UPLOAD_CONSUMERS,
    publish_after_media_upload,
    "after_media_upload",
);

/// 照片及其文件删除后发布的事件，供缓存等后续服务消费。
pub struct AfterMediaDelete {
    pub medias: Vec<MediaRecord>,
}

step_derive::declare_async_event!(
    crate::state::MediaState,
    AfterMediaDelete,
    AFTER_MEDIA_DELETE_CONSUMERS,
    publish_after_media_delete,
    "after_media_delete",
);
