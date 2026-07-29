use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

use crate::error::ParseIdError;

// ============================================================
// PhotoId
// ============================================================

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(type = "string"))]
pub struct PhotoId(pub i64);

/// 序列化为字符串（如 "42"），而非数字
impl Serialize for PhotoId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

/// 反序列化时同时接受字符串 ("42") 和数字 (42)
impl<'de> Deserialize<'de> for PhotoId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PhotoIdVisitor;

        impl<'de> Visitor<'de> for PhotoIdVisitor {
            type Value = PhotoId;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a photo ID as a number or string")
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<PhotoId, E> {
                Ok(PhotoId(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<PhotoId, E> {
                Ok(PhotoId(v as i64))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<PhotoId, E> {
                v.parse::<i64>()
                    .map(PhotoId)
                    .map_err(|_| de::Error::custom("PhotoId 格式错误"))
            }
        }

        d.deserialize_any(PhotoIdVisitor)
    }
}

impl From<i64> for PhotoId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<PhotoId> for i64 {
    fn from(id: PhotoId) -> Self {
        id.0
    }
}

impl fmt::Display for PhotoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for PhotoId {
    type Err = ParseIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<i64>()
            .map(PhotoId)
            .map_err(|_| ParseIdError("无效 photo_id"))
    }
}

impl PhotoId {
    pub fn parse_from_str_or_none(s: &str) -> Option<Self> {
        let id = s.parse::<i64>().ok()?;
        Some(Self(id))
    }
}

// ============================================================
// SeaORM 支持（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
impl From<PhotoId> for sea_orm::Value {
    fn from(val: PhotoId) -> Self {
        sea_orm::Value::BigInt(Some(val.0))
    }
}

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::auth::user::UserId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_photo")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub user_id: i64,
        pub name: String,
        pub size: i64,
        pub width: i32,
        pub height: i32,
        pub mime_type: String,
        pub md5: String,
        pub file_id: String,
        pub comment_count: i64,
        pub like_count: i64,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    /// 照片记录，使用强类型 ID
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct PhotoRecord {
        pub id: PhotoId,
        pub user_id: UserId,
        pub name: String,
        pub size: i64,
        pub width: i32,
        pub height: i32,
        pub mime_type: String,
        pub md5: String,
        pub file_id: String,
        pub comment_count: u64,
        pub like_count: u64,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    impl From<Model> for PhotoRecord {
        fn from(model: Model) -> Self {
            Self {
                id: PhotoId(model.id),
                user_id: UserId(model.user_id),
                name: model.name,
                size: model.size,
                width: model.width,
                height: model.height,
                mime_type: model.mime_type,
                md5: model.md5,
                file_id: model.file_id,
                comment_count: model.comment_count as u64,
                like_count: model.like_count as u64,
                created_at: model.created_at,
                updated_at: model.updated_at,
            }
        }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;
