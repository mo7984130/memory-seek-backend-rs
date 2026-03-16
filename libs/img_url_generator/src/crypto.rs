use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const NONCE: &[u8; 12] = b"img_token_12";

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("加密失败")]
    EncryptError,
    #[error("解密失败")]
    DecryptError,
    #[error("无效的token")]
    InvalidToken,
}

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

/// 加密图片 Token
pub fn encrypt_image_token(token: &ImageToken, key: &[u8; 32]) -> Result<String, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::EncryptError)?;
    
    let nonce = Nonce::from_slice(NONCE);
    
    let json = serde_json::to_string(token).map_err(|_| CryptoError::EncryptError)?;
    
    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|_| CryptoError::EncryptError)?;
    
    Ok(URL_SAFE_NO_PAD.encode(&ciphertext))
}

/// 解密图片 Token
pub fn decrypt_image_token(token: &str, key: &[u8; 32]) -> Result<ImageToken, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::DecryptError)?;
    
    let nonce = Nonce::from_slice(NONCE);
    
    let ciphertext = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| CryptoError::InvalidToken)?;
    
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|_| CryptoError::DecryptError)?;
    
    let json = String::from_utf8(plaintext).map_err(|_| CryptoError::DecryptError)?;
    
    serde_json::from_str(&json).map_err(|_| CryptoError::DecryptError)
}

/// 使用 AES-256-GCM 加密 file_id（兼容旧接口）
pub fn encrypt_file_id(file_id: &str, key: &[u8; 32]) -> Result<String, CryptoError> {
    let token = ImageToken::thumbnail(file_id.to_string());
    encrypt_image_token(&token, key)
}

/// 解密 token 获取 file_id（兼容旧接口）
pub fn decrypt_file_id(token: &str, key: &[u8; 32]) -> Result<String, CryptoError> {
    let image_token = decrypt_image_token(token, key)?;
    Ok(image_token.file_id)
}

/// 加密人脸封面 Token（兼容旧接口）
pub fn encrypt_face_cover_token(
    file_id: &str,
    bbox: &FaceBBoxPixels,
    key: &[u8; 32],
) -> Result<String, CryptoError> {
    let token = ImageToken::crop(file_id.to_string(), *bbox);
    encrypt_image_token(&token, key)
}

/// 解密人脸封面 Token（兼容旧接口）
pub fn decrypt_face_cover_token(token: &str, key: &[u8; 32]) -> Result<ImageToken, CryptoError> {
    decrypt_image_token(token, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_token_thumbnail() {
        let key = b"01234567890123456789012345678901";
        let token = ImageToken::thumbnail("photos/2024/01/abc.jpg".to_string());
        
        let encrypted = encrypt_image_token(&token, key).unwrap();
        let decrypted = decrypt_image_token(&encrypted, key).unwrap();
        
        assert_eq!(token.file_id, decrypted.file_id);
        assert_eq!(ImageTokenType::Thumbnail, decrypted.token_type);
        assert!(decrypted.bbox.is_none());
    }

    #[test]
    fn test_image_token_crop() {
        let key = b"01234567890123456789012345678901";
        let bbox = FaceBBoxPixels { x: 100, y: 200, w: 300, h: 400 };
        let token = ImageToken::crop("photos/2024/01/abc.jpg".to_string(), bbox);
        
        let encrypted = encrypt_image_token(&token, key).unwrap();
        let decrypted = decrypt_image_token(&encrypted, key).unwrap();
        
        assert_eq!(token.file_id, decrypted.file_id);
        assert_eq!(ImageTokenType::Crop, decrypted.token_type);
        assert!(decrypted.bbox.is_some());
        
        let decrypted_bbox = decrypted.bbox.unwrap();
        assert_eq!(bbox.x, decrypted_bbox.x);
        assert_eq!(bbox.y, decrypted_bbox.y);
        assert_eq!(bbox.w, decrypted_bbox.w);
        assert_eq!(bbox.h, decrypted_bbox.h);
    }
}
