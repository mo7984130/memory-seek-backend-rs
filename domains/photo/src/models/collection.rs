use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use common::models::ImageToken;
use entities::photo::{collection::CollectionRecord, photo::PhotoId};
use img_url_generator::TokenCipher;
use sea_orm::entity::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};

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

impl From<CollectionRecord> for CollectionVO {
    fn from(record: CollectionRecord) -> Self {
        CollectionVO {
            id: record.id.0.to_string(),
            name: record.name,
            description: record.description,
            photo_count: record.photo_count,
            cover_token: None,
            is_favorite: record.is_favorite,
            created_at: record.created_at,
        }
    }
}

impl CollectionVO {
    pub fn with_generate_cover_token(mut self, cipher: &TokenCipher) -> Self {
        self.cover_token = self.cover_token.as_ref().and_then(|fid| {
            cipher
                .encrypt(&ImageToken::thumbnail(fid.to_string()), None)
                .ok()
        });
        self
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCreateParma {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionUpdateParam {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoCursorPageQuery {
    pub cursor: Option<String>,
    pub size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionPhotoCursor {
    pub created_at: DateTimeUtc,
    pub id: PhotoId,
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
pub struct CollectionPhotoAddBatchParam {
    pub photo_ids: Vec<PhotoId>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoAddBatchResult {
    pub new_photo_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoRemoveBatchParam {
    pub photo_ids: Vec<PhotoId>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoRemoveBatchResult {
    pub removed_photo_count: u64,
}
