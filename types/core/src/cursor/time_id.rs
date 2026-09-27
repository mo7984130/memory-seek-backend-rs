//! `(time, id)` 复合游标:适用于 `ORDER BY created_at, id` 的分页场景。

use base64::Engine;
use common_core::time::DateTime;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{CursorDecodeError, KeysetDirection};

/// 通用时间+ID 复合游标，适用于 `(created_at, id)` 排序的分页场景。
///
/// 编码为 URL-safe Base64（JSON → base64），用于 API 透传。
/// 序列化时直接输出编码后的 Base64 字符串，反序列化时接收 Base64 字符串。
#[derive(Debug, Clone)]
pub struct TimeIdCursor<I = i64> {
    pub time_at: DateTime,
    pub id: I,
}

impl<I: Serialize> TimeIdCursor<I> {
    /// 编码为 URL-safe Base64 字符串
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(&serde_json::json!({
            "time_at": self.time_at,
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
            time_at: DateTime,
            id: I,
        }
        let raw: Raw<I> = serde_json::from_str(&json).map_err(CursorDecodeError::Json)?;
        Ok(Self {
            time_at: raw.time_at,
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
            KeysetDirection::Desc => Condition::any().add(time_col.lt(self.time_at)).add(
                Condition::all()
                    .add(time_col.eq(self.time_at))
                    .add(id_col.lt(self.id.clone())),
            ),
            KeysetDirection::Asc => Condition::any().add(time_col.gt(self.time_at)).add(
                Condition::all()
                    .add(time_col.eq(self.time_at))
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
