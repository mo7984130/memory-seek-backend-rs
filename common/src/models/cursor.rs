use std::fmt::Debug;

use base64::Engine;
use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, Condition, Value};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::cursor_error::CursorDecodeError;

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
/// 所有现有业务中具有相同结构的游标（`PhotoCursor`、`CollectionPhotoCursor`、`PhotoLikeCursor`）
/// 都应统一替换为此类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeIdCursor<I = i64> {
    pub created_at: DateTime<Utc>,
    pub id: I,
}

impl<I: Serialize + DeserializeOwned> TimeIdCursor<I> {
    /// 编码为 URL-safe Base64 字符串
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    /// 从 URL-safe Base64 字符串解码
    pub fn decode(s: impl AsRef<[u8]>) -> std::result::Result<Self, CursorDecodeError> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s.as_ref())
            .map_err(CursorDecodeError::Base64)?;
        let json = String::from_utf8(bytes).map_err(CursorDecodeError::Utf8)?;
        serde_json::from_str(&json).map_err(CursorDecodeError::Json)
    }
}

impl<I: Clone + Into<Value>> TimeIdCursor<I> {
    /// 构造 keyset 分页过滤条件, 需与 `ORDER BY time <dir>, id <dir>` 配套使用。
    ///
    /// - `Desc`: 返回 `(time, id) < (self.created_at, self.id)` 的行(向前翻页)
    /// - `Asc`: 返回 `(time, id) > (self.created_at, self.id)` 的行(向后翻页)
    pub fn keyset_condition<C: ColumnTrait>(
        &self,
        time_col: C,
        id_col: C,
        direction: KeysetDirection,
    ) -> Condition {
        match direction {
            KeysetDirection::Desc => Condition::any()
                .add(time_col.lt(self.created_at))
                .add(
                    Condition::all()
                        .add(time_col.eq(self.created_at))
                        .add(id_col.lt(self.id.clone())),
                ),
            KeysetDirection::Asc => Condition::any()
                .add(time_col.gt(self.created_at))
                .add(
                    Condition::all()
                        .add(time_col.eq(self.created_at))
                        .add(id_col.gt(self.id.clone())),
                ),
        }
    }

    /// 便捷方法: `Desc` 方向, 见 [`TimeIdCursor::keyset_condition`]
    pub fn before<C: ColumnTrait>(&self, time_col: C, id_col: C) -> Condition {
        self.keyset_condition(time_col, id_col, KeysetDirection::Desc)
    }

    /// 便捷方法: `Asc` 方向, 见 [`TimeIdCursor::keyset_condition`]
    pub fn after<C: ColumnTrait>(&self, time_col: C, id_col: C) -> Condition {
        self.keyset_condition(time_col, id_col, KeysetDirection::Asc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use sea_orm::{
        DbBackend, QueryFilter, QueryTrait,
        entity::prelude::*,
    };

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "test_entity")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub created_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

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

    // ============ keyset 过滤条件 ============

    fn cursor() -> TimeIdCursor<i64> {
        TimeIdCursor {
            created_at: DateTime::from_timestamp_nanos(1712345678000000000),
            id: 42,
        }
    }

    fn build_sql(cond: Condition) -> String {
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
}
