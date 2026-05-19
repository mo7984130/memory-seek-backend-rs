//! 照片域数据访问层（Mapper）
//!
//! 封装各业务实体的数据库操作，包括收藏、评论、点赞、人脸特征、
//! 人物、照片及时间线统计等 CRUD 与批量查询方法。

pub mod collection_mapper;
pub mod collection_photo_mapper;
mod comment_like_mapper;
pub mod comment_mapper;
#[cfg(feature = "face_recognition")]
pub mod face_feature_mapper;
#[cfg(feature = "face_recognition")]
pub mod face_person_mapper;
pub mod photo_mapper;
mod timeline_stat_mapper;

pub use collection_mapper::CollectionMapper;
pub use collection_photo_mapper::CollectionPhotoMapper;
pub use comment_like_mapper::CommentLikeMapper;
pub use comment_mapper::CommentMapper;
#[cfg(feature = "face_recognition")]
pub use face_feature_mapper::FaceFeatureMapper;
#[cfg(feature = "face_recognition")]
pub use face_person_mapper::FacePersonMapper;
pub use photo_mapper::PhotoMapper;
pub use timeline_stat_mapper::TimelineStatMapper;
