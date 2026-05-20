use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use entities::collection;
use img_url_generator::TokenCipher;
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};

use crate::photo::PhotoVO;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CollectionVO {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub photo_count: i64,
    pub cover_token: Option<String>,
    pub is_favorite: bool,
    pub created_at: DateTimeUtc,
}

impl CollectionVO {
    pub fn from_collection(c: collection::Model, cipher: &TokenCipher) -> Self {
        CollectionVO {
            id: c.id.to_string(),
            name: c.name,
            description: c.description,
            photo_count: c.photo_count,
            cover_token: c
                .cover_file_id
                .as_ref()
                .and_then(|fid| PhotoVO::generate_thumbnail_token(fid, cipher)),
            is_favorite: c.is_favorite,
            created_at: c.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCreateDTO {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionEditDTO {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoVO {
    pub photo: super::photo::PhotoVO,
    pub collected_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoQuery {
    pub cursor: Option<String>,
    pub size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionPhotoCursor {
    pub created_at: DateTime<Utc>,
    pub id: i64,
}

impl CollectionPhotoCursor {
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
    /// 解码成功返回 `Some(CollectionPhotoCursor)`，失败返回 `None`
    pub fn decode(s: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(s).ok()?;
        let json = String::from_utf8(bytes).ok()?;
        serde_json::from_str(&json).ok()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPhotosDTO {
    pub photo_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchOperationResultVO {
    pub success_count: u32,
    pub already_exists_count: u32,
    pub already_exists_photo_ids: Vec<String>,
    pub failed_count: u32,
    pub failed_photo_ids: Vec<String>,
}
