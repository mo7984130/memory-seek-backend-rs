use std::fmt::Debug;

use base64::Engine;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::CursorDecodeError;

/// keyset 分页排序方向, 需与查询的 `ORDER BY` 保持一致
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeysetDirection {
    /// 倒序: `ORDER BY time DESC, id DESC`, 游标取 `(time, id) < cursor`
    Desc,
    /// 正序: `ORDER BY time ASC, id ASC`, 游标取 `(time, id) > cursor`
    Asc,
}

/// 通用时间+ID 复合游标，适用于 `(created_at, id)` 排序的分页场景。
///
/// 编码为 URL-safe Base64（JSON → base64），用于 API 透传。
/// 序列化时直接输出编码后的 Base64 字符串，反序列化时接收 Base64 字符串。
#[derive(Debug, Clone)]
pub struct TimeIdCursor<I = i64> {
    pub created_at: DateTime<Utc>,
    pub id: I,
}

impl<I: Serialize> TimeIdCursor<I> {
    /// 编码为 URL-safe Base64 字符串
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(&serde_json::json!({
            "created_at": self.created_at,
            "id": &self.id,
        }))
        .unwrap_or_default();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes())
    }
}

impl<I: DeserializeOwned> TimeIdCursor<I> {
    /// 从 URL-safe Base64 字符串解码
    pub fn decode(s: impl AsRef<[u8]>) -> std::result::Result<Self, CursorDecodeError> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s.as_ref())
            .map_err(CursorDecodeError::Base64)?;
        let json = String::from_utf8(bytes).map_err(CursorDecodeError::Utf8)?;

        #[derive(Deserialize)]
        struct Raw<I> {
            created_at: DateTime<Utc>,
            id: I,
        }
        let raw: Raw<I> = serde_json::from_str(&json).map_err(CursorDecodeError::Json)?;
        Ok(Self {
            created_at: raw.created_at,
            id: raw.id,
        })
    }
}

impl<I: Serialize> Serialize for TimeIdCursor<I> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de, I: DeserializeOwned> Deserialize<'de> for TimeIdCursor<I> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::decode(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl<I> validator::Validate for TimeIdCursor<I> {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        Ok(())
    }
}

/// 通用"计数+ID"复合游标, 用于按计数(如 `face_count`)主排序的 keyset 分页。
///
/// 编码为 URL-safe Base64(JSON → base64), 用于 API 透传。
/// 序列化时直接输出编码后的 Base64 字符串, 反序列化时接收 Base64 字符串。
/// 用法与 [`TimeIdCursor`] 一致: `ORDER BY face_count <dir>, id <dir>`,
/// keyset 条件 `(face_count, id) < cursor`。
#[derive(Debug, Clone)]
pub struct FaceCountIdCursor<I = i64> {
    pub face_count: u64,
    pub id: I,
}

impl<I: Serialize> FaceCountIdCursor<I> {
    /// 编码为 URL-safe Base64 字符串
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(&serde_json::json!({
            "face_count": self.face_count,
            "id": &self.id,
        }))
        .unwrap_or_default();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes())
    }
}

impl<I: DeserializeOwned> FaceCountIdCursor<I> {
    /// 从 URL-safe Base64 字符串解码
    pub fn decode(s: impl AsRef<[u8]>) -> std::result::Result<Self, CursorDecodeError> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s.as_ref())
            .map_err(CursorDecodeError::Base64)?;
        let json = String::from_utf8(bytes).map_err(CursorDecodeError::Utf8)?;

        #[derive(Deserialize)]
        struct Raw<I> {
            face_count: u64,
            id: I,
        }
        let raw: Raw<I> = serde_json::from_str(&json).map_err(CursorDecodeError::Json)?;
        Ok(Self {
            face_count: raw.face_count,
            id: raw.id,
        })
    }
}

impl<I: Serialize> Serialize for FaceCountIdCursor<I> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de, I: DeserializeOwned> Deserialize<'de> for FaceCountIdCursor<I> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::decode(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl<I> validator::Validate for FaceCountIdCursor<I> {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        Ok(())
    }
}

#[cfg(feature = "orm")]
impl<I: Clone + Into<sea_orm::Value>> TimeIdCursor<I> {
    /// 构造 keyset 分页过滤条件, 需与 `ORDER BY time <dir>, id <dir>` 配套使用。
    ///
    /// - `Desc`: 返回 `(time, id) < (self.created_at, self.id)` 的行(向前翻页)
    /// - `Asc`: 返回 `(time, id) > (self.created_at, self.id)` 的行(向后翻页)
    pub fn keyset_condition<C: sea_orm::ColumnTrait>(
        &self,
        time_col: C,
        id_col: C,
        direction: KeysetDirection,
    ) -> sea_orm::Condition {
        use sea_orm::Condition;
        match direction {
            KeysetDirection::Desc => Condition::any().add(time_col.lt(self.created_at)).add(
                Condition::all()
                    .add(time_col.eq(self.created_at))
                    .add(id_col.lt(self.id.clone())),
            ),
            KeysetDirection::Asc => Condition::any().add(time_col.gt(self.created_at)).add(
                Condition::all()
                    .add(time_col.eq(self.created_at))
                    .add(id_col.gt(self.id.clone())),
            ),
        }
    }

    /// 便捷方法: `Desc` 方向, 见 [`TimeIdCursor::keyset_condition`]
    pub fn before<C: sea_orm::ColumnTrait>(&self, time_col: C, id_col: C) -> sea_orm::Condition {
        self.keyset_condition(time_col, id_col, KeysetDirection::Desc)
    }

    /// 便捷方法: `Asc` 方向, 见 [`TimeIdCursor::keyset_condition`]
    pub fn after<C: sea_orm::ColumnTrait>(&self, time_col: C, id_col: C) -> sea_orm::Condition {
        self.keyset_condition(time_col, id_col, KeysetDirection::Asc)
    }
}

#[cfg(feature = "orm")]
impl<I: Clone + Into<sea_orm::Value>> FaceCountIdCursor<I> {
    /// 构造 keyset 分页过滤条件, 需与 `ORDER BY face_count <dir>, id <dir>` 配套使用。
    ///
    /// - `Desc`: 返回 `(face_count, id) < (self.face_count, self.id)` 的行
    /// - `Asc`: 返回 `(face_count, id) > (self.face_count, self.id)` 的行
    pub fn keyset_condition<C: sea_orm::ColumnTrait>(
        &self,
        count_col: C,
        id_col: C,
        direction: KeysetDirection,
    ) -> sea_orm::Condition {
        use sea_orm::Condition;
        let face_count = self.face_count as i64;
        match direction {
            KeysetDirection::Desc => Condition::any().add(count_col.lt(face_count)).add(
                Condition::all()
                    .add(count_col.eq(face_count))
                    .add(id_col.lt(self.id.clone())),
            ),
            KeysetDirection::Asc => Condition::any().add(count_col.gt(face_count)).add(
                Condition::all()
                    .add(count_col.eq(face_count))
                    .add(id_col.gt(self.id.clone())),
            ),
        }
    }

    /// 便捷方法: `Desc` 方向, 见 [`FaceCountIdCursor::keyset_condition`]
    pub fn before<C: sea_orm::ColumnTrait>(&self, count_col: C, id_col: C) -> sea_orm::Condition {
        self.keyset_condition(count_col, id_col, KeysetDirection::Desc)
    }

    /// 便捷方法: `Asc` 方向, 见 [`FaceCountIdCursor::keyset_condition`]
    pub fn after<C: sea_orm::ColumnTrait>(&self, count_col: C, id_col: C) -> sea_orm::Condition {
        self.keyset_condition(count_col, id_col, KeysetDirection::Asc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestId(i64);

    #[test]
    fn test_encode_decode_roundtrip() {
        let cursor = TimeIdCursor {
            created_at: DateTime::from_timestamp_nanos(1712345678000000000),
            id: TestId(42),
        };

        let encoded = cursor.encode();
        let decoded = TimeIdCursor::<TestId>::decode(&encoded).unwrap();

        assert_eq!(decoded.created_at, cursor.created_at);
        assert_eq!(decoded.id.0, cursor.id.0);
    }

    #[test]
    fn test_decode_invalid_base64() {
        let result = TimeIdCursor::<i64>::decode("!!!invalid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_empty_string() {
        let result = TimeIdCursor::<i64>::decode("");
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_is_url_safe_no_pad() {
        let cursor = TimeIdCursor {
            created_at: DateTime::from_timestamp_nanos(1712345678000000000),
            id: 42i64,
        };

        let encoded = cursor.encode();
        // URL_SAFE_NO_PAD 不包含 + / = 字符
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
    }

    #[test]
    fn test_serialize_outputs_base64_string() {
        let cursor = TimeIdCursor {
            created_at: DateTime::from_timestamp_nanos(1712345678000000000),
            id: 42i64,
        };

        let json = serde_json::to_string(&cursor).unwrap();
        // 序列化为 Base64 字符串, 而非对象
        assert!(json.starts_with('"') && json.ends_with('"'), "json: {json}");
        assert_eq!(json.trim_matches('"'), cursor.encode());

        let decoded: TimeIdCursor<i64> = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.created_at, cursor.created_at);
        assert_eq!(decoded.id, cursor.id);
    }

    #[test]
    fn test_face_count_cursor_encode_decode_roundtrip() {
        let cursor = FaceCountIdCursor {
            face_count: 7,
            id: TestId(42),
        };

        let encoded = cursor.encode();
        let decoded = FaceCountIdCursor::<TestId>::decode(&encoded).unwrap();

        assert_eq!(decoded.face_count, cursor.face_count);
        assert_eq!(decoded.id.0, cursor.id.0);
    }

    #[test]
    fn test_face_count_cursor_serialize_outputs_base64_string() {
        let cursor = FaceCountIdCursor {
            face_count: 7,
            id: 42i64,
        };

        let json = serde_json::to_string(&cursor).unwrap();
        assert!(json.starts_with('"') && json.ends_with('"'), "json: {json}");
        assert_eq!(json.trim_matches('"'), cursor.encode());

        let decoded: FaceCountIdCursor<i64> = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.face_count, cursor.face_count);
        assert_eq!(decoded.id, cursor.id);
    }

    #[test]
    fn test_face_count_cursor_decode_invalid_base64() {
        let result = FaceCountIdCursor::<i64>::decode("!!!invalid-base64!!!");
        assert!(result.is_err());
    }

    #[cfg(feature = "orm")]
    mod keyset_tests {
        use super::*;
        use chrono::DateTime;
        use sea_orm::{entity::prelude::*, DbBackend, QueryFilter, QueryTrait};

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "test_entity")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i64,
            pub created_at: DateTimeUtc,
            pub face_count: i64,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}

        fn cursor() -> TimeIdCursor<i64> {
            TimeIdCursor {
                created_at: DateTime::from_timestamp_nanos(1712345678000000000),
                id: 42,
            }
        }

        fn build_sql(cond: sea_orm::Condition) -> String {
            Entity::find()
                .filter(cond)
                .build(DbBackend::Postgres)
                .to_string()
        }

        #[test]
        fn test_keyset_desc_before() {
            let sql = build_sql(cursor().before(Column::CreatedAt, Column::Id));
            // (created_at, id) < cursor: created_at < ? OR (created_at = ? AND id < 42)
            assert!(
                sql.contains("\"created_at\" < ") && sql.contains("\"created_at\" = "),
                "before sql: {sql}"
            );
            // 时间相等时的 tiebreaker 必须用 id
            let eq_pos = sql.find("\"created_at\" = ").expect("equal branch");
            assert!(
                sql[eq_pos..].contains("\"id\" < 42"),
                "before tiebreaker: {sql}"
            );
        }

        #[test]
        fn test_keyset_asc_after() {
            let sql = build_sql(cursor().after(Column::CreatedAt, Column::Id));
            // (created_at, id) > cursor: created_at > ? OR (created_at = ? AND id > 42)
            assert!(
                sql.contains("\"created_at\" > ") && sql.contains("\"created_at\" = "),
                "after sql: {sql}"
            );
            let eq_pos = sql.find("\"created_at\" = ").expect("equal branch");
            assert!(
                sql[eq_pos..].contains("\"id\" > 42"),
                "after tiebreaker: {sql}"
            );
        }

        #[test]
        fn test_keyset_direction_matches_keyset_condition() {
            assert_eq!(
                build_sql(cursor().before(Column::CreatedAt, Column::Id)),
                build_sql(cursor().keyset_condition(
                    Column::CreatedAt,
                    Column::Id,
                    KeysetDirection::Desc
                ))
            );
            assert_eq!(
                build_sql(cursor().after(Column::CreatedAt, Column::Id)),
                build_sql(cursor().keyset_condition(
                    Column::CreatedAt,
                    Column::Id,
                    KeysetDirection::Asc
                ))
            );
        }

        #[test]
        fn test_face_count_cursor_keyset_desc_before() {
            let cursor = FaceCountIdCursor {
                face_count: 7,
                id: 42,
            };
            let sql = build_sql(cursor.before(Column::FaceCount, Column::Id));
            // (face_count, id) < cursor: face_count < 7 OR (face_count = 7 AND id < 42)
            assert!(
                sql.contains("\"face_count\" < ") && sql.contains("\"face_count\" = "),
                "before sql: {sql}"
            );
            let eq_pos = sql.find("\"face_count\" = ").expect("equal branch");
            assert!(
                sql[eq_pos..].contains("\"id\" < 42"),
                "before tiebreaker: {sql}"
            );
        }

        #[test]
        fn test_face_count_cursor_keyset_asc_after() {
            let cursor = FaceCountIdCursor {
                face_count: 7,
                id: 42,
            };
            let sql = build_sql(cursor.after(Column::FaceCount, Column::Id));
            assert!(
                sql.contains("\"face_count\" > ") && sql.contains("\"face_count\" = "),
                "after sql: {sql}"
            );
            let eq_pos = sql.find("\"face_count\" = ").expect("equal branch");
            assert!(
                sql[eq_pos..].contains("\"id\" > 42"),
                "after tiebreaker: {sql}"
            );
        }

        #[test]
        fn test_face_count_cursor_keyset_direction_matches_keyset_condition() {
            let cursor = FaceCountIdCursor {
                face_count: 7,
                id: 42,
            };
            assert_eq!(
                build_sql(cursor.before(Column::FaceCount, Column::Id)),
                build_sql(cursor.keyset_condition(
                    Column::FaceCount,
                    Column::Id,
                    KeysetDirection::Desc
                ))
            );
            assert_eq!(
                build_sql(cursor.after(Column::FaceCount, Column::Id)),
                build_sql(cursor.keyset_condition(
                    Column::FaceCount,
                    Column::Id,
                    KeysetDirection::Asc
                ))
            );
        }
    }
}
