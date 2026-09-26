//! `(count, id)` 复合游标:适用于 `ORDER BY <count>, id` 的分页场景。

use base64::Engine;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{CursorDecodeError, KeysetDirection};

/// 通用"计数+ID"复合游标, 用于按计数(如 `face_count`)主排序的 keyset 分页。
///
/// 编码为 URL-safe Base64(JSON → base64), 用于 API 透传。
/// 序列化时直接输出编码后的 Base64 字符串, 反序列化时接收 Base64 字符串。
/// 用法与 [`TimeIdCursor`](super::TimeIdCursor) 一致: `ORDER BY face_count <dir>, id <dir>`,
/// keyset 条件 `(face_count, id) < cursor`。
#[derive(Debug, Clone)]
pub struct CountIdCursor<I = i64> {
    pub count: u64,
    pub id: I,
}

impl<I: Serialize> CountIdCursor<I> {
    /// 编码为 URL-safe Base64 字符串
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(&serde_json::json!({
            "face_count": self.count,
            "id": &self.id,
        }))
        .unwrap_or_default();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes())
    }
}

impl<I: DeserializeOwned> CountIdCursor<I> {
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
            count: raw.face_count,
            id: raw.id,
        })
    }
}

impl<I: Serialize> Serialize for CountIdCursor<I> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de, I: DeserializeOwned> Deserialize<'de> for CountIdCursor<I> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::decode(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl<I> validator::Validate for CountIdCursor<I> {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        Ok(())
    }
}

#[cfg(feature = "orm")]
impl<I: Clone + Into<sea_orm::Value>> CountIdCursor<I> {
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
        let face_count = self.count as i64;
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

    /// 便捷方法: `Desc` 方向, 见 [`CountIdCursor::keyset_condition`]
    pub fn before<C: sea_orm::ColumnTrait>(&self, count_col: C, id_col: C) -> sea_orm::Condition {
        self.keyset_condition(count_col, id_col, KeysetDirection::Desc)
    }

    /// 便捷方法: `Asc` 方向, 见 [`CountIdCursor::keyset_condition`]
    pub fn after<C: sea_orm::ColumnTrait>(&self, count_col: C, id_col: C) -> sea_orm::Condition {
        self.keyset_condition(count_col, id_col, KeysetDirection::Asc)
    }
}
