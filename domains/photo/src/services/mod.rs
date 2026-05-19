//! 照片域服务层
//!
//! 提供照片管理、收藏、评论、人脸/特征识别和时间线统计等业务逻辑。

pub mod photo_service;
pub mod collection_service;
#[cfg(feature = "face_recognition")]
pub mod face_service;
#[cfg(feature = "face_recognition")]
pub mod feature_service;
pub mod comment_service;
pub mod timeline_stat_service;

pub use collection_service::CollectionService;
pub use comment_service::CommentService;
#[cfg(feature = "face_recognition")]
pub use face_service::FaceService;
#[cfg(feature = "face_recognition")]
pub use feature_service::FeatureService;
pub use photo_service::PhotoService;
pub use timeline_stat_service::TimelineStatService;
