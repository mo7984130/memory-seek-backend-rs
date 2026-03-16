use chrono::{DateTime, Utc};
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
    pub created_at: DateTime<Utc>,
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
    pub cursor: Option<DateTime<Utc>>,
    pub size: Option<u32>,
}
