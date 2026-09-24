//! 游标编解码与 keyset 条件测试。
//!
//! 两个游标共用 `test_entity` 夹具与同一套 `ORDER BY …` 语义, 因此测试集中在本文件,
//! 而不是各游标一份。

use super::*;
use common_core::time::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestId(i64);

#[test]
fn test_encode_decode_roundtrip() {
    let cursor = TimeIdCursor {
        time_at: DateTime::from_timestamp_nanos(1712345678000000000),
        id: TestId(42),
    };

    let encoded = cursor.encode();
    let decoded = TimeIdCursor::<TestId>::decode(&encoded).unwrap();

    assert_eq!(decoded.time_at, cursor.time_at);
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
        time_at: DateTime::from_timestamp_nanos(1712345678000000000),
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
        time_at: DateTime::from_timestamp_nanos(1712345678000000000),
        id: 42i64,
    };

    let json = serde_json::to_string(&cursor).unwrap();
    // 序列化为 Base64 字符串, 而非对象
    assert!(json.starts_with('"') && json.ends_with('"'), "json: {json}");
    assert_eq!(json.trim_matches('"'), cursor.encode());

    let decoded: TimeIdCursor<i64> = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.time_at, cursor.time_at);
    assert_eq!(decoded.id, cursor.id);
}

#[test]
fn test_face_count_cursor_encode_decode_roundtrip() {
    let cursor = CountIdCursor {
        count: 7,
        id: TestId(42),
    };

    let encoded = cursor.encode();
    let decoded = CountIdCursor::<TestId>::decode(&encoded).unwrap();

    assert_eq!(decoded.count, cursor.count);
    assert_eq!(decoded.id.0, cursor.id.0);
}

#[test]
fn test_face_count_cursor_serialize_outputs_base64_string() {
    let cursor = CountIdCursor {
        count: 7,
        id: 42i64,
    };

    let json = serde_json::to_string(&cursor).unwrap();
    assert!(json.starts_with('"') && json.ends_with('"'), "json: {json}");
    assert_eq!(json.trim_matches('"'), cursor.encode());

    let decoded: CountIdCursor<i64> = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.count, cursor.count);
    assert_eq!(decoded.id, cursor.id);
}

#[test]
fn test_face_count_cursor_decode_invalid_base64() {
    let result = CountIdCursor::<i64>::decode("!!!invalid-base64!!!");
    assert!(result.is_err());
}

#[cfg(feature = "orm")]
mod keyset_tests {
    use super::*;
    use common_core::time::DateTime as CommonDateTime;
    use sea_orm::{DbBackend, QueryFilter, QueryTrait, entity::prelude::*};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "test_entity")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub created_at: CommonDateTime,
        pub face_count: i64,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    fn cursor() -> TimeIdCursor<i64> {
        TimeIdCursor {
            time_at: CommonDateTime::from_timestamp_nanos(1712345678000000000),
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
        let cursor = CountIdCursor { count: 7, id: 42 };
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
        let cursor = CountIdCursor { count: 7, id: 42 };
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
        let cursor = CountIdCursor { count: 7, id: 42 };
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
            build_sql(cursor.keyset_condition(Column::FaceCount, Column::Id, KeysetDirection::Asc))
        );
    }
}
