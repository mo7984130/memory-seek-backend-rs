mod error;
pub use error::OssError;

mod retry;
use retry::retry_429;

use bytes::Bytes;
use futures::{Stream, StreamExt};
use s3::creds::Credentials;
use s3::request::ResponseData;
use s3::{Bucket, Region};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

static CHUNK_SIZE: usize = 256;
static CONCURRENCY: usize = 16;

#[derive(Clone)]
pub struct S3Client {
    bucket: Arc<Bucket>,
    public_url: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct S3Config {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
    pub public_url: Option<String>,
    pub force_path_style: bool,
}

impl S3Client {
    /// 根据配置创建 S3 客户端
    ///
    /// # 参数
    /// - `s3_config`: S3 连接配置，包含端点、凭证、区域、桶名等
    ///
    /// # 返回
    /// 初始化完成的 `S3Client` 实例
    pub fn new(s3_config: &S3Config) -> Self {
        let region = Region::Custom {
            region: s3_config.region.clone(),
            endpoint: s3_config.endpoint.clone(),
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
            bucket: Arc::from(bucket),
            public_url: s3_config
                .public_url
                .clone()
                .unwrap_or_else(|| s3_config.endpoint.clone())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    /// 上传文件到 OSS
    ///
    /// # 参数
    /// - `key`: 文件路径/键名
    /// - `data`: 文件内容（实现 `AsRef<[u8]>` 的类型）
    /// - `content_type`: MIME 类型，如 "image/jpeg"
    ///
    /// # 返回
    /// 上传成功返回 `()`
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: OSS 存储操作失败
    pub async fn upload(
        &self,
        key: &str,
        data: impl AsRef<[u8]>,
        content_type: &str,
    ) -> Result<ResponseData, OssError> {
        retry_429(key, || async {
            self.bucket
                .put_object_with_content_type(key, data.as_ref(), content_type)
                .await
                .map_err(OssError::from)
        })
        .await
    }

    /// 删除单个文件
    ///
    /// # 参数
    /// - `key`: 文件路径/键名
    ///
    /// # 返回
    /// 删除成功返回 `()`
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: OSS 删除操作失败
    pub async fn delete(&self, key: &str) -> Result<ResponseData, OssError> {
        retry_429(key, || async {
            self.bucket.delete_object(key).await.map_err(OssError::from)
        })
        .await
    }

    /// 批量删除文件，分片并发执行，遇错即停
    ///
    /// # 参数
    /// - `keys`: 待删除的文件路径/键名列表
    ///
    /// # 返回
    /// 全部删除成功返回 `()`
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: 删除文件失败
    pub async fn delete_batch(&self, keys: Vec<impl AsRef<str>>) -> Result<(), OssError> {
        for concurrent_chunks in keys.chunks(CHUNK_SIZE * CONCURRENCY) {
            let futures: Vec<_> = concurrent_chunks
                .chunks(CHUNK_SIZE)
                .map(|chunk| async move {
                    for key in chunk {
                        retry_429(key.as_ref(), || async {
                            self.bucket
                                .delete_object(key.as_ref())
                                .await
                                .map_err(OssError::from)
                        })
                        .await?;
                    }
                    Ok::<_, OssError>(())
                })
                .collect();

            futures::future::try_join_all(futures).await?;
        }

        Ok(())
    }

    /// 获取文件的公开访问 URL
    ///
    /// # 参数
    /// - `key`: 文件路径/键名
    ///
    /// # 返回
    /// 拼接公开域名后的完整 URL
    pub fn get_url(&self, key: &str) -> String {
        format!("{}/{}", self.public_url, key.trim_start_matches('/'))
    }

    /// 获取文件的签名 URL（无图片处理参数）
    ///
    /// # 参数
    /// - `key`: 文件路径/键名
    /// - `expires`: 签名有效期
    ///
    /// # 返回
    /// 带签名的临时访问 URL
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: OSS 签名生成失败
    pub async fn get_signed_url(&self, key: &str, expires: Duration) -> Result<String, OssError> {
        self.get_signed_url_with_params(key, expires, None).await
    }

    /// 获取带图片处理参数的签名 URL
    ///
    /// # 参数
    /// - `key`: 文件路径/键名
    /// - `expires`: 签名有效期
    /// - `process`: OSS 图片处理参数，如 "image/resize,w_300"，为 `None` 时不附加处理参数
    ///
    /// # 返回
    /// 带签名和图片处理参数的临时访问 URL
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: OSS 签名生成失败
    pub async fn get_signed_url_with_params(
        &self,
        key: &str,
        expires: Duration,
        process: Option<String>,
    ) -> Result<String, OssError> {
        let custom_queries = if let Some(p) = process {
            let mut queries = HashMap::new();
            queries.insert("x-oss-process".to_string(), p);
            Some(queries)
        } else {
            None
        };
        self.bucket
            .presign_get(key, expires.as_secs() as u32, custom_queries)
            .await
            .map_err(OssError::from)
    }

    /// 下载文件
    ///
    /// # 参数
    /// - `key`: 文件路径/键名
    ///
    /// # 返回
    /// 文件内容的 `Bytes`，可直接用于 HTTP 响应
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: OSS 下载操作失败
    pub async fn download(&self, key: &str) -> Result<Bytes, OssError> {
        let response_data = retry_429(key, || async {
            self.bucket.get_object(key).await.map_err(OssError::from)
        })
        .await?;

        Ok(response_data.into_bytes())
    }

    pub async fn get_download_stream_response(
        &self,
        key: &str,
    ) -> Result<impl Stream<Item = Result<Bytes, OssError>> + use<>, OssError> {
        let response = retry_429(key, || async {
            self.bucket
                .get_object_stream(key)
                .await
                .map_err(OssError::from)
        })
        .await?;
        Ok(response.bytes.map(|item| item.map_err(OssError::from)))
    }

    pub async fn download_with_process(&self, key: &str, process: &str) -> Result<Bytes, OssError> {
        let url = self
            .get_signed_url_with_params(key, Duration::from_secs(3600), Some(process.to_string()))
            .await?;

        retry_429(key, || async {
            let response = reqwest::get(&url).await?;
            if !response.status().is_success() {
                return Err(OssError::from_response(response).await);
            }

            response.bytes().await.map_err(OssError::from)
        })
        .await
    }

    pub async fn download_stream_with_process(
        &self,
        key: &str,
        process: &str,
    ) -> Result<impl Stream<Item = Result<Bytes, OssError>> + use<>, OssError> {
        let url = self
            .get_signed_url_with_params(key, Duration::from_secs(3600), Some(process.to_string()))
            .await?;

        let response = retry_429(key, || async {
            let response = reqwest::get(&url).await?;
            if !response.status().is_success() {
                return Err(OssError::from_response(response).await);
            }
            Ok(response)
        })
        .await?;

        let bytes = response.bytes_stream().map(|r| r.map_err(OssError::from));

        Ok(bytes)
    }
}
