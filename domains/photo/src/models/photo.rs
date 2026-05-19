use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use common::models::ImageToken;
use common::utils::TokenCipher;
use common::{error::AppError, utils::ResultExt};
use sea_orm::FromQueryResult;
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
use crate::services::photo_service::PageDirection;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhotoVO {
    pub id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub size: i64,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_favorited: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_collected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_token: Option<String>,
}

impl PhotoVO {
    /// 为照片生成缩略图、预览和原图三种加密访问令牌
    ///
    /// # 参数
    /// - `file_id`: 文件唯一标识
    /// - `token_cipher`: 加密器实例
    ///
    /// # 返回
    /// 返回 `(thumbnail_token, preview_token, original_token)` 三元组，加密失败时对应位置为 `None`
    pub fn generate_tokens(
        file_id: &str,
        token_cipher: &TokenCipher,
    ) -> (Option<String>, Option<String>, Option<String>) {
        let thumbnail_token = token_cipher
            .encrypt(&ImageToken::thumbnail(file_id.to_string()), Some(file_id))
            .ok();
        let preview_token = token_cipher
            .encrypt(&ImageToken::preview(file_id.to_string()), Some(file_id))
            .ok();
        let original_token = token_cipher
            .encrypt(&ImageToken::original(file_id.to_string()), Some(file_id))
            .ok();

        (thumbnail_token, preview_token, original_token)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoCursorQuery {
    pub cursor: Option<String>,
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(default = "default_direction")]
    pub direction: PageDirection,
    pub default_collection_id: Option<String>,
}

// 默认分页大小
fn default_size() -> u32 {
    100
}

// 默认翻页方向
fn default_direction() -> PageDirection {
    PageDirection::Next
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoCursor {
    pub created_at: DateTime<Utc>,
    pub id: i64,
}

impl PhotoCursor {
    /// 将游标编码为 URL 安全的 Base64 字符串
    ///
    /// # 返回
    /// 返回 Base64 编码后的游标字符串
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    /// 从 URL 安全的 Base64 字符串解码游标
    ///
    /// # 参数
    /// - `s`: Base64 编码的游标字符串
    ///
    /// # 返回
    /// 返回解码后的 `PhotoCursor`
    ///
    /// # 错误
    /// - `AppError`: Base64 解码失败、UTF-8 转换失败或 JSON 反序列化失败
    pub fn decode(s: impl AsRef<[u8]>) -> Result<Self, AppError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s)
            .trace_bad_request_err("photo::photo_cursor:decode_err", "解码photo_curosr错误")?;
        let json = String::from_utf8(bytes)
            .trace_bad_request_err("photo::photo_cursor:from_utf8_err", "解码photo_curosr错误")?;
        serde_json::from_str(&json)
            .trace_bad_request_err("photo::photo_cursor:from_str_err", "解码photo_curosr错误")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPageVO<T, C> {
    pub records: Vec<T>,
    pub next_cursor: Option<C>,
    pub has_more: bool,
}

impl<T, C> CursorPageVO<T, C> {
    /// 创建空的游标分页容器
    ///
    /// # 返回
    /// 返回无记录、无游标的空分页结果
    pub fn empty() -> Self {
        Self {
            records: vec![],
            next_cursor: None,
            has_more: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadWithCreatedAtQuery {
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5Query {
    pub md5: Vec<String>,
}

#[derive(Serialize, FromQueryResult)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    pub min_time: Option<DateTimeUtc>,
    pub max_time: Option<DateTimeUtc>,
}
impl Default for TimeRange {
    /// 创建时间范围实例，最小和最大时间均为 `None`
    fn default() -> Self {
        Self {
            min_time: None,
            max_time: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoInfo {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub width: i32,
    pub height: i32,
    pub mime_type: String,
    pub created_at: DateTimeUtc,
}
impl From<entities::photo::Model> for PhotoInfo {
    /// 从数据库照片实体转换为照片信息 DTO
    fn from(m: entities::photo::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            size: m.size,
            width: m.width,
            height: m.height,
            mime_type: m.mime_type,
            created_at: m.created_at,
        }
    }
}
