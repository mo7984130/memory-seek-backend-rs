use std::fmt::Debug;

use base64::Engine;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::cursor_error::CursorDecodeError;

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
}
