use crate::{ImageUrl, ImageUrlGenerator};
use async_trait::async_trait;
use oss::S3Client;
use std::sync::Arc;
use std::time::Duration;

const CACHE_AGE: u32 = 604800;

pub struct AliyunOssGenerator {
    pub s3_client: Arc<S3Client>,
}

#[async_trait]
impl ImageUrlGenerator for AliyunOssGenerator {
    /// 生成缩略图 URL（300x300，WebP 格式）
    ///
    /// # 参数
    /// - `file_id`: OSS 中的文件路径/键名
    ///
    /// # 返回
    /// 包含签名 URL 和缓存时长的 `ImageUrl`
    async fn thumbnail(&self, file_id: &str) -> ImageUrl {
        let process = "image/resize,m_fill,w_300,h_300/format,webp";
        let url = self.s3_client
            .get_signed_url_with_params(file_id, Duration::from_secs(CACHE_AGE as u64), Some(process.to_string()))
            .await
            .unwrap_or_default();
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }

    /// 生成预览图 URL（宽度 1200px，WebP 格式）
    ///
    /// # 参数
    /// - `file_id`: OSS 中的文件路径/键名
    ///
    /// # 返回
    /// 包含签名 URL 和缓存时长的 `ImageUrl`
    async fn preview(&self, file_id: &str) -> ImageUrl {
        let process = "image/resize,w_1200/format,webp";
        let url = self.s3_client
            .get_signed_url_with_params(file_id, Duration::from_secs(CACHE_AGE as u64), Some(process.to_string()))
            .await
            .unwrap_or_default();
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }

    /// 生成原图 URL（无图片处理）
    ///
    /// # 参数
    /// - `file_id`: OSS 中的文件路径/键名
    /// - `_extension`: 文件扩展名（OSS 模式下未使用）
    ///
    /// # 返回
    /// 包含签名 URL 和缓存时长的 `ImageUrl`
    async fn original(&self, file_id: &str, _extension: &str) -> ImageUrl {
        let url = self.s3_client
            .get_signed_url(file_id, Duration::from_secs(CACHE_AGE as u64))
            .await
            .unwrap_or_default();
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }

    /// 生成裁剪图 URL（指定区域裁剪后缩放为正方形，WebP 格式）
    ///
    /// # 参数
    /// - `file_id`: OSS 中的文件路径/键名
    /// - `x`: 裁剪区域左上角 X 坐标
    /// - `y`: 裁剪区域左上角 Y 坐标
    /// - `w`: 裁剪区域宽度
    /// - `h`: 裁剪区域高度
    /// - `size`: 输出正方形的边长
    ///
    /// # 返回
    /// 包含签名 URL 和缓存时长的 `ImageUrl`
    async fn crop(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> ImageUrl {
        let process = format!("image/crop,x_{},y_{},w_{},h_{}/resize,m_fill,w_{},h_{}/format,webp", x, y, w, h, size, size);
        let url = self.s3_client
            .get_signed_url_with_params(file_id, Duration::from_secs(CACHE_AGE as u64), Some(process))
            .await
            .unwrap_or_default();
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }
}
