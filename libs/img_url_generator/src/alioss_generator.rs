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
    async fn thumbnail(&self, file_id: &str) -> ImageUrl {
        let process = "image/resize,m_fill,w_300,h_300/format,webp";
        let url = self.s3_client
            .get_signed_url_with_params(file_id, Duration::from_secs(CACHE_AGE as u64), Some(process.to_string()))
            .await
            .unwrap_or_default();
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }

    async fn preview(&self, file_id: &str) -> ImageUrl {
        let process = "image/resize,w_1200/format,webp";
        let url = self.s3_client
            .get_signed_url_with_params(file_id, Duration::from_secs(CACHE_AGE as u64), Some(process.to_string()))
            .await
            .unwrap_or_default();
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }

    async fn original(&self, file_id: &str, _extension: &str) -> ImageUrl {
        let url = self.s3_client
            .get_signed_url(file_id, Duration::from_secs(CACHE_AGE as u64))
            .await
            .unwrap_or_default();
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }

    async fn crop(&self, file_id: &str, x: i32, y: i32, w: i32, h: i32, size: u32) -> ImageUrl {
        let process = format!("image/crop,x_{},y_{},w_{},h_{}/resize,m_fill,w_{},h_{}/format,webp", x, y, w, h, size, size);
        let url = self.s3_client
            .get_signed_url_with_params(file_id, Duration::from_secs(CACHE_AGE as u64), Some(process))
            .await
            .unwrap_or_default();
        
        ImageUrl { url, cache_age: CACHE_AGE }
    }
}
