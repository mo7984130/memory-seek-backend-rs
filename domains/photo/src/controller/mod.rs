/// 照片域控制器模块
///
/// 包含照片、收藏、评论和时间线相关的 HTTP 请求处理器。
/// 通过 feature flag `face_recognition` 可选编译人脸识别控制器。
pub mod photo_controller;
pub mod collection_controller;
pub mod comment_controller;
pub mod timeline_controller;
#[cfg(feature = "face_recognition")]
pub mod face_controller;

pub use photo_controller::PhotoController;
pub use collection_controller::CollectionController;
pub use comment_controller::CommentController;
pub use timeline_controller::TimelineController;
#[cfg(feature = "face_recognition")]
pub use face_controller::FaceController;
