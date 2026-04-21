use serde::{Deserialize, Serialize};

/// 图片类型
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ImageTokenType {
    Thumbnail,
    Preview,
    Original,
    Crop,
}

/// 人脸边界框（绝对像素坐标）
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct FaceBBoxPixels {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 统一图片 Token
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImageToken {
    /// 文件路径
    pub file_id: String,
    /// 图片类型
    #[serde(rename = "type")]
    pub token_type: ImageTokenType,
    /// 人脸边界框（仅 Crop 类型需要）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<FaceBBoxPixels>,
}

impl ImageToken {
    /// 创建缩略图 token
    pub fn thumbnail(file_id: String) -> Self {
        Self {
            file_id,
            token_type: ImageTokenType::Thumbnail,
            bbox: None,
        }
    }

    /// 创建预览图 token
    pub fn preview(file_id: String) -> Self {
        Self {
            file_id,
            token_type: ImageTokenType::Preview,
            bbox: None,
        }
    }

    /// 创建原图 token
    pub fn original(file_id: String) -> Self {
        Self {
            file_id,
            token_type: ImageTokenType::Original,
            bbox: None,
        }
    }

    /// 创建裁剪图 token（人脸封面）
    pub fn crop(file_id: String, bbox: FaceBBoxPixels) -> Self {
        Self {
            file_id,
            token_type: ImageTokenType::Crop,
            bbox: Some(bbox),
        }
    }
}
