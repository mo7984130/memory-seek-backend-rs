#[cfg(feature = "face")]
use common_core::TempFile;
use types_visual::visual::VisualRecord;

/// 影像主记录落库后发布的事件，供时间线、人脸等后续服务消费。
pub struct AfterVisualUpload {
    pub visual: VisualRecord,
    /// 上传流式落盘的临时文件, 供启用 `face` 后的人脸识别订阅者消费;
    /// 消费完成(无论成败)后由守卫自动删除。
    #[cfg(feature = "face")]
    pub temp_file: TempFile,
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
