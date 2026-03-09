use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use oss::S3Client;
use crate::ImageUrlGenerator;

pub struct AliyunOssGenerator {
    pub s3_client: Arc<S3Client>,
}

#[async_trait]
impl ImageUrlGenerator for AliyunOssGenerator {
    async fn thumbnail(&self, file_id: String) -> String {
        let process = "image/resize,m_fill,w_300,h_300/format,webp";
        self.s3_client
            .get_signed_url_with_params(&file_id, Duration::from_secs(1800), Option::from(process.to_string()))
            .await
            .unwrap_or_default()
    }

    async fn preview(&self, file_id: String) -> String {
        let process = "image/resize,w_1200/format,webp";
        self.s3_client
            .get_signed_url_with_params(&file_id, Duration::from_secs(1800), process.to_string().into() )
            .await
            .unwrap_or_default()
    }

    async fn original(&self, file_id: String, _extension: String) -> String {
        self.s3_client
            .get_signed_url(&file_id, Duration::from_secs(3600))
            .await
            .unwrap_or_default()
    }

    async fn crop(&self, file_id: String, x: i32, y: i32, w: i32, h: i32, size: u32) -> String {
        let process = format!("image/crop,x_{},y_{},w_{},h_{}/resize,m_fill,w_{},h_{}/format,webp", x, y, w, h, size, size);
        self.s3_client
            .get_signed_url_with_params(&file_id, Duration::from_secs(1800), process.into())
            .await
            .unwrap_or_default()
    }
}