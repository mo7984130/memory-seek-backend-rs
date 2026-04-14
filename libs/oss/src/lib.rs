use bytes::Bytes;
use common::error::AppError;
use common::utils::ResultExt;
use s3::creds::Credentials;
use s3::{Bucket, Region};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct S3Client {
    bucket: Arc<Bucket>,
    public_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
    pub public_url: String,
    pub force_path_style: bool,
}

impl S3Client {
    pub async fn new(s3_config: S3Config) -> Self {
        let region = Region::Custom {
            region: s3_config.region,
            endpoint: s3_config.endpoint,
        };

        let access_key = s3_config.access_key.clone();
        let secret_key = s3_config.secret_key.clone();
        
        let credentials = Credentials::new(
            Some(access_key.as_str()),
            Some(secret_key.as_str()),
            None,
            None,
            None,
        )
        .expect("Failed to create S3 credentials");

        let bucket = Bucket::new(&s3_config.bucket, region, credentials)
            .expect("Failed to create S3 bucket");

        let bucket = if s3_config.force_path_style {
            bucket.with_path_style()
        } else {
            bucket
        };

        Self {
            bucket: Arc::new(*bucket),
            public_url: s3_config.public_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn upload(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<(), AppError> {
        self.bucket
            .put_object_with_content_type(key, &data, content_type)
            .await
            .map_internal_err("OSS文件存储失败")?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<(), AppError> {
        self.bucket
            .delete_object(key)
            .await
            .map_internal_err("OSS文件删除失败")?;
        Ok(())
    }

    pub fn get_url(&self, key: &str) -> String {
        format!("{}/{}", self.public_url, key.trim_start_matches('/'))
    }

    pub async fn get_signed_url(&self, key: &str, expires: Duration) -> Result<String, AppError> {
        self.get_signed_url_with_params(key, expires, None).await
    }

    pub async fn get_signed_url_with_params(
        &self,
        key: &str,
        expires: Duration,
        process: Option<String>,
    ) -> Result<String, AppError> {
        let custom_queries = if let Some(p) = process {
            let mut queries = HashMap::new();
            queries.insert("x-oss-process".to_string(), p);
            Some(queries)
        } else {
            None
        };

        let url = self
            .bucket
            .presign_get(key, expires.as_secs() as u32, custom_queries)
            .await
            .map_internal_err("OSS 签名失败")?;

        Ok(url)
    }

    pub async fn download(&self, key: &str) -> Result<Bytes, AppError> {
        let response_data = self
            .bucket
            .get_object(key)
            .await
            .map_internal_err("OSS下载失败")?;

        Ok(Bytes::from(response_data.bytes().to_vec()))
    }

    pub async fn download_with_process(&self, key: &str, process: &str) -> Result<Bytes, AppError> {
        let url = format!("{}?x-oss-process={}", self.get_url(key), process);
        
        let response = reqwest::get(&url)
            .await
            .map_internal_err("OSS下载失败")?;
        
        let bytes = response.bytes().await.map_internal_err("读取响应失败")?;

        Ok(bytes)
    }
}
