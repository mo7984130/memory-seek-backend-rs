#[cfg(feature = "face")]
use bytes::Bytes;
use types::visual::visual::VisualRecord;

/// 影像主记录落库后发布的事件，供时间线、人脸等后续服务消费。
pub struct AfterVisualUpload {
    pub visual: VisualRecord,
    /// 保留原始字节，供启用 `face` 后的人脸识别订阅者消费。
    #[cfg(feature = "face")]
    pub file_data: Bytes,
}

step_derive::declare_async_event!(
    crate::state::VisualState,
    AfterVisualUpload,
    AFTER_MEDIA_UPLOAD_CONSUMERS,
    publish_after_visual_upload,
    "after_visual_upload",
);

/// 影像及其文件删除后发布的事件，供缓存等后续服务消费。
pub struct AfterVisualDelete {
    pub visuals: Vec<VisualRecord>,
}

step_derive::declare_async_event!(
    crate::state::VisualState,
    AfterVisualDelete,
    AFTER_MEDIA_DELETE_CONSUMERS,
    publish_after_visual_delete,
    "after_visual_delete",
);
