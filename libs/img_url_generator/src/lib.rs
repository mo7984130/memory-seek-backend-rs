mod alioss_generator;
mod crypto;
mod imgproxy_generator;

use std::sync::Arc;
use async_trait::async_trait;
use serde::Deserialize;

use crate::alioss_generator::AliyunOssGenerator;
use crate::imgproxy_generator::ImgProxyGenerator;
use oss::S3Client;

pub use crypto::{
    decrypt_file_id, encrypt_file_id, 
    encrypt_face_cover_token, decrypt_face_cover_token,
    encrypt_image_token, decrypt_image_token,
    CryptoError, ImageToken, ImageTokenType, FaceBBoxPixels,
};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    ImgProxy,
    Oss,
}

#[derive(Deserialize, Clone)]
pub struct ImageUrlGeneratorConfig {
    pub oss_url: String,
    pub proxy_type: ProxyType,
    pub key: Option<String>,
    pub salt: Option<String>,
    pub bucket: String,
    pub encryption_key: String,
}

#[derive(Clone)]
pub struct ImageUrl {
    pub url: String,
    pub cache_age: u32,
}

#[async_trait]
pub trait ImageUrlGenerator: Send + Sync {
    async fn thumbnail(&self, file_id: &str) -> ImageUrl;
    async fn preview(&self, file_id: &str) -> ImageUrl;
    async fn original(&self, file_id: &str, extension: &str) -> ImageUrl;
    async fn crop(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> ImageUrl;
}

pub enum ImageUrlProvider {
    ImgProxy(ImgProxyGenerator),
    AliyunOss(AliyunOssGenerator),
}

impl ImageUrlProvider {
    pub fn new(config: ImageUrlGeneratorConfig, s3_client: Option<Arc<S3Client>>) -> Self {
        match config.proxy_type {
            ProxyType::ImgProxy => {
                let key_hex = config.key.expect("【配置错误】ImgProxy 模式必须配置 KEY");
                let salt_hex = config.salt.expect("【配置错误】ImgProxy 模式必须配置 SALT");

                Self::ImgProxy(ImgProxyGenerator {
                    base_url: config.oss_url,
                    key: hex::decode(key_hex).expect("【格式错误】ImgProxy KEY 必须是合法的 Hex"),
                    salt: hex::decode(salt_hex).expect("【格式错误】ImgProxy SALT 必须是合法的 Hex"),
                    bucket: config.bucket,
                })
            }
            ProxyType::Oss => {
                let client = s3_client.expect("【配置错误】使用 Oss 模式必须注入 S3Client 实例");
                Self::AliyunOss(AliyunOssGenerator {
                    s3_client: client,
                })
            }
        }
    }
    
    /// 生成加密的图片 token
    /// 
    /// # 参数
    /// - `file_id`: 文件路径
    /// 
    /// # 返回
    /// 加密后的 token 字符串
    pub fn generate_token(&self, file_id: &str, encryption_key: &[u8; 32]) -> String {
        encrypt_file_id(file_id, encryption_key).unwrap_or_default()
    }
    
    /// 解密 token 获取 file_id
    /// 
    /// # 参数
    /// - `token`: 加密的 token
    /// 
    /// # 返回
    /// 解密后的文件路径
    pub fn decrypt_token(&self, token: &str, encryption_key: &[u8; 32]) -> Result<String, CryptoError> {
        decrypt_file_id(token, encryption_key)
    }
}

#[async_trait]
impl ImageUrlGenerator for ImageUrlProvider {
    async fn thumbnail(&self, file_id: &str) -> ImageUrl {
        match self {
            Self::ImgProxy(generator) => generator.thumbnail(file_id).await,
            Self::AliyunOss(generator) => generator.thumbnail(file_id).await,
        }
    }

    async fn preview(&self, file_id: &str) -> ImageUrl {
        match self {
            Self::ImgProxy(generator) => generator.preview(file_id).await,
            Self::AliyunOss(generator) => generator.preview(file_id).await,
        }
    }

    async fn original(&self, file_id: &str, extension: &str) -> ImageUrl {
        match self {
            Self::ImgProxy(generator) => generator.original(file_id, extension).await,
            Self::AliyunOss(generator) => generator.original(file_id, extension).await,
        }
    }

    async fn crop(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> ImageUrl {
        match self {
            Self::ImgProxy(generator) => generator.crop(file_id, x, y, w, h, size).await,
            Self::AliyunOss(generator) => generator.crop(file_id, x, y, w, h, size).await,
        }
    }
}
