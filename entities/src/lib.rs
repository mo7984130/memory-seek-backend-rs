//! 数据库实体定义模块
//!
//! 使用 Sea-ORM 框架定义所有数据库表结构及其关联关系，涵盖：
//! - `photo_entities`：照片、集合、评论、点赞、人脸特征、人脸人物、时间线统计
//! - `user_entities`：用户
//! - `vector`：PostgreSQL pgvector 向量类型的 Sea-ORM 适配层

pub mod vector;
pub mod photo_entities;
pub mod user_entities;

pub use vector::PostgreVector;

pub use photo_entities::collection;
pub use photo_entities::collection_photo;
pub use photo_entities::comment;
pub use photo_entities::comment_like;
pub use photo_entities::face_feature;
pub use photo_entities::face_feature::Embedding512;
pub use photo_entities::face_person;
pub use photo_entities::photo;
pub use photo_entities::timeline_stat;

pub use user_entities::user;
