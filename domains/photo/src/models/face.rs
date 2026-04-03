use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FacePersonVO {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_photo_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_token: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FacePersonSimpleVO {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FaceFeatureVO {
    pub id: String,
    pub person_id: Option<String>,
    pub person_name: String,
    pub bbox: FaceBBox,
    pub score: f32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FaceBBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePersonRequest {
    pub new_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePersonRequest {
    pub source_person_id: String,
    pub target_person_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonPageQuery {
    pub cursor: Option<String>,
    pub size: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonSearchQuery {
    pub keyword: String,
    pub cursor: Option<String>,
    pub size: Option<u32>,
    #[serde(default)]
    pub detailed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonCursor {
    pub total_photo_count: i64,
    pub id: i64,
}

impl PersonCursor {
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }

    pub fn decode(s: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(s).ok()?;
        let json = String::from_utf8(bytes).ok()?;
        serde_json::from_str(&json).ok()
    }
}

#[derive(Debug, Clone)]
pub struct FeatureNode {
    pub id: i64,
    pub photo_id: i64,
    pub embedding: Vec<f32>,
    pub score: f32,
    pub person_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PersonCluster {
    pub id: i64,
    pub vector: Vec<f32>,
    /// 存储成员特征的 ID 列表（避免存储完整的 FeatureNode，节省内存）
    pub member_ids: Vec<i64>,
    pub total_weight: f32,
}
