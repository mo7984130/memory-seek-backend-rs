use bytes::Bytes;
use common::error::AppError;
use common::ext::ResultErrExt;
use futures::{Stream, StreamExt};
use s3::creds::Credentials;
use s3::request::ResponseDataStream;
use s3::{Bucket, Region};
use serde::Deserialize;
use std::collections::HashMap;
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
    ) -> Result<(), AppError> {
        self.bucket
            .put_object_with_content_type(key, data.as_ref(), content_type)
            .await
            .trace_internal_err("oss_upload_err", "OSS文件存储失败")?;
        Ok(())
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
    pub async fn delete(&self, key: &str) -> Result<(), AppError> {
        self.bucket
            .delete_object(key)
            .await
            .trace_internal_err("oss_delete_err", "OSS文件删除失败")?;
        Ok(())
    }

    /// 批量删除文件，分片并发执行
    ///
    /// # 参数
    /// - `keys`: 待删除的文件路径/键名列表
    ///
    /// # 返回
    /// 全部删除成功返回 `()`
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: 部分文件删除失败
    pub async fn delete_batch(&self, keys: Vec<String>) -> Result<(), AppError> {
        let mut failed_keys: Vec<&str> = Vec::new();

        for concurrent_chunks in keys.chunks(CHUNK_SIZE * CONCURRENCY) {
            let futures: Vec<_> = concurrent_chunks
                .chunks(CHUNK_SIZE)
                .map(|chunk| async move {
                    let mut chunk_failed: Vec<&str> = Vec::new();
                    for key in chunk {
                        if let Err(e) = self
                            .bucket
                            .delete_object(key.as_str())
                            .await
                            .trace_internal_err("oss_del_err", "OSS文件删除失败")
                        {
                            tracing::warn!(key = %key, err = ?e, "文件删除失败");
                            chunk_failed.push(key.as_str());
                        }
                    }
                    chunk_failed
                })
                .collect();

            let results = futures::future::join_all(futures).await;
            for chunk_failed in results {
                failed_keys.extend(chunk_failed);
            }
        }

        if failed_keys.is_empty() {
            Ok(())
        } else {
            tracing::error!(
                keys = ?failed_keys,
                count = failed_keys.len(),
                "批量删除部分文件失败"
            );
            Err(AppError::InternalServerError)
        }
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
    pub async fn get_signed_url(&self, key: &str, expires: Duration) -> Result<String, AppError> {
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
            .trace_internal_err("oss_sign_url_err", "OSS 签名失败")?;

        Ok(url)
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
    pub async fn download(&self, key: &str) -> Result<Bytes, AppError> {
        let response_data = self
            .bucket
            .get_object(key)
            .await
            .trace_internal_err("oss_download_err", "OSS下载失败")?;

        Ok(Bytes::from(response_data.bytes().to_vec()))
    }

    pub async fn get_download_stream_response(
        &self,
        key: &str,
    ) -> Result<ResponseDataStream, AppError> {
        self.bucket
            .get_object_stream(key)
            .await
            .trace_internal_err("oss_download_err", "OSS流下载失败")
    }

    // pub async fn download_stream(
    //     &self,
    //     key: String,
    // ) -> Result<impl Stream<Item = Result<Bytes, AppError>>, AppError> {
    //     let response = self
    //         .bucket
    //         .get_object_stream(key)
    //         .await
    //         .trace_internal_err("oss_download_err", "OSS流下载失败")?;

    //     let stream = response
    //         .bytes
    //         .map(|chunk| chunk.trace_internal_err("oss_stream_err", "OSS流读取失败"));

    //     Ok(stream)
    // }

    pub async fn download_with_process(&self, key: &str, process: &str) -> Result<Bytes, AppError> {
        let mut custom_queries = HashMap::new();
        custom_queries.insert("x-oss-process".to_string(), process.to_string());

        let url = self
            .bucket
            .presign_get(key, 3600, Some(custom_queries))
            .await
            .trace_internal_err("oss_sign_url_err", "OSS签名失败")?;

        let response = reqwest::get(&url)
            .await
            .trace_internal_err("oss_download_err", "OSS下载失败")?
            .error_for_status()
            .trace_internal_err("oss_bad_status_err", "OSS返回了错误状态码")?;

        let bytes = response
            .bytes()
            .await
            .trace_internal_err("oss_read_data_err", "OSS读取数据失败")?;

        Ok(bytes)
    }

    pub async fn download_stream_with_process(
        &self,
        key: &str,
        process: &str,
    ) -> Result<impl Stream<Item = Result<Bytes, AppError>>, AppError> {
        let mut custom_queries = HashMap::new();
        custom_queries.insert("x-oss-process".to_string(), process.to_string());

        let url = self
            .bucket
            .presign_get(key, 3600, Some(custom_queries))
            .await
            .trace_internal_err("oss_sign_url_err", "OSS签名失败")?;

        let response = reqwest::get(&url)
            .await
            .trace_internal_err("oss_download_err", "OSS下载失败")?
            .error_for_status()
            .trace_internal_err("oss_bad_status_err", "OSS返回了错误状态码")?;

        let bytes = response
            .bytes_stream()
            .map(|r| r.trace_internal_err("oss_read_data_err", "OSS读取数据失败"));

        Ok(bytes)
    }
}
