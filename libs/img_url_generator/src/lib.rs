mod alioss_generator;
mod crypto;
mod imgproxy_generator;

use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;

use crate::alioss_generator::AliyunOssGenerator;
use crate::imgproxy_generator::ImgProxyGenerator;
use oss::S3Client;

pub use crypto::CryptoError;

pub use common::utils::TokenCipher;

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
}

#[derive(Clone)]
pub struct ImageUrl {
    pub url: String,
    pub cache_age: u32,
}

/// 图片 URL 生成器 trait，统一缩略图、预览、原图、裁剪的 URL 生成接口
#[async_trait]
pub trait ImageUrlGenerator: Send + Sync {
    /// 生成缩略图 URL
    ///
    /// # 参数
    /// - `file_id`: 文件路径/键名
    ///
    /// # 返回
    /// 包含 URL 和缓存时长的 `ImageUrl`
    async fn thumbnail(&self, file_id: &str) -> ImageUrl;

    /// 生成预览图 URL
    ///
    /// # 参数
    /// - `file_id`: 文件路径/键名
    ///
    /// # 返回
    /// 包含 URL 和缓存时长的 `ImageUrl`
    async fn preview(&self, file_id: &str) -> ImageUrl;

    /// 生成原图 URL
    ///
    /// # 参数
    /// - `file_id`: 文件路径/键名
    /// - `extension`: 文件扩展名
    ///
    /// # 返回
    /// 包含 URL 和缓存时长的 `ImageUrl`
    async fn original(&self, file_id: &str, extension: &str) -> ImageUrl;

    /// 生成裁剪图 URL
    ///
    /// # 参数
    /// - `file_id`: 文件路径/键名
    /// - `x`: 裁剪区域左上角 X 坐标
    /// - `y`: 裁剪区域左上角 Y 坐标
    /// - `w`: 裁剪区域宽度
    /// - `h`: 裁剪区域高度
    /// - `size`: 输出正方形的边长
    ///
    /// # 返回
    /// 包含 URL 和缓存时长的 `ImageUrl`
    async fn crop(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> ImageUrl;
}

pub enum ImageUrlProvider {
    ImgProxy(ImgProxyGenerator),
    AliyunOss(AliyunOssGenerator),
}

impl ImageUrlProvider {
    /// 根据配置创建图片 URL 提供者
    ///
    /// # 参数
    /// - `config`: 图片 URL 生成器配置，包含代理类型、OSS 地址、密钥等
    /// - `s3_client`: S3 客户端实例，OSS 模式下必须提供
    ///
    /// # 返回
    /// 对应代理类型的 `ImageUrlProvider` 实例
    pub fn new(config: ImageUrlGeneratorConfig, s3_client: Option<Arc<S3Client>>) -> Self {
        match config.proxy_type {
            ProxyType::ImgProxy => {
                let key_hex = config.key.expect("【配置错误】ImgProxy 模式必须配置 KEY");
                let salt_hex = config.salt.expect("【配置错误】ImgProxy 模式必须配置 SALT");

                Self::ImgProxy(ImgProxyGenerator {
                    base_url: config.oss_url,
                    key: hex::decode(key_hex).expect("【格式错误】ImgProxy KEY 必须是合法的 Hex"),
                    salt: hex::decode(salt_hex)
                        .expect("【格式错误】ImgProxy SALT 必须是合法的 Hex"),
                    bucket: config.bucket,
                })
            }
            ProxyType::Oss => {
                let client = s3_client.expect("【配置错误】使用 Oss 模式必须注入 S3Client 实例");
                Self::AliyunOss(AliyunOssGenerator { s3_client: client })
            }
        }
    }
}

#[async_trait]
impl ImageUrlGenerator for ImageUrlProvider {
    /// 生成缩略图 URL，委托给内部代理实现
    ///
    /// # 参数
    /// - `file_id`: 文件路径/键名
    ///
    /// # 返回
    /// 包含 URL 和缓存时长的 `ImageUrl`
    async fn thumbnail(&self, file_id: &str) -> ImageUrl {
        match self {
            Self::ImgProxy(generator) => generator.thumbnail(file_id).await,
            Self::AliyunOss(generator) => generator.thumbnail(file_id).await,
        }
    }

    /// 生成预览图 URL，委托给内部代理实现
    ///
    /// # 参数
    /// - `file_id`: 文件路径/键名
    ///
    /// # 返回
    /// 包含 URL 和缓存时长的 `ImageUrl`
    async fn preview(&self, file_id: &str) -> ImageUrl {
        match self {
            Self::ImgProxy(generator) => generator.preview(file_id).await,
            Self::AliyunOss(generator) => generator.preview(file_id).await,
        }
    }

    /// 生成原图 URL，委托给内部代理实现
    ///
    /// # 参数
    /// - `file_id`: 文件路径/键名
    /// - `extension`: 文件扩展名
    ///
    /// # 返回
    /// 包含 URL 和缓存时长的 `ImageUrl`
    async fn original(&self, file_id: &str, extension: &str) -> ImageUrl {
        match self {
            Self::ImgProxy(generator) => generator.original(file_id, extension).await,
            Self::AliyunOss(generator) => generator.original(file_id, extension).await,
        }
    }

    /// 生成裁剪图 URL，委托给内部代理实现
    ///
    /// # 参数
    /// - `file_id`: 文件路径/键名
    /// - `x`: 裁剪区域左上角 X 坐标
    /// - `y`: 裁剪区域左上角 Y 坐标
    /// - `w`: 裁剪区域宽度
    /// - `h`: 裁剪区域高度
    /// - `size`: 输出正方形的边长
    ///
    /// # 返回
    /// 包含 URL 和缓存时长的 `ImageUrl`
    async fn crop(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> ImageUrl {
        match self {
            Self::ImgProxy(generator) => generator.crop(file_id, x, y, w, h, size).await,
            Self::AliyunOss(generator) => generator.crop(file_id, x, y, w, h, size).await,
        }
    }
}
