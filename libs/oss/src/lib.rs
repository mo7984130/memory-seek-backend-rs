mod error;
pub use error::OssError;

mod retry;
#[cfg(feature = "metrics")]
use retry::metric_name;
use retry::retry_429;

use bytes::Bytes;
use common_core::time::Duration;
use futures::{Stream, StreamExt};
use s3::creds::Credentials;
use s3::request::ResponseData;
use s3::{Bucket, Region};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

static CHUNK_SIZE: usize = 256;
static CONCURRENCY: usize = 16;

/// S3 客户端内部共享状态.
struct S3ClientInner {
    bucket: Bucket,
    public_url: String,
    endpoint: String,
}

#[derive(Clone)]
pub struct S3Client {
    inner: Arc<S3ClientInner>,
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

/// 流式下载响应: 状态码、范围信息与字节流
pub struct DownloadStream {
    /// 对象存储返回的状态码(200 全量 / 206 部分内容 / 416 范围越界)
    pub status: u16,
    /// 对象元数据中的 `Content-Type`
    pub content_type: Option<String>,
    /// `Content-Range` 响应头(仅 206 / 416 时存在)
    pub content_range: Option<String>,
    /// 本次响应体的字节数
    pub content_length: Option<u64>,
    /// 响应体字节流
    pub stream: Pin<Box<dyn Stream<Item = Result<Bytes, OssError>> + Send>>,
}

impl S3Client {
    /// 根据配置创建 S3 客户端；凭证或桶初始化失败时 panic
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
            *bucket.with_path_style()
        } else {
            *bucket
        };

        Self {
            inner: Arc::new(S3ClientInner {
                bucket,
                public_url: s3_config
                    .public_url
                    .clone()
                    .unwrap_or_else(|| s3_config.endpoint.clone())
                    .trim_end_matches('/')
                    .to_string(),
                endpoint: s3_config.endpoint.clone(),
            }),
        }
    }

    /// 上传文件到 OSS
    pub async fn upload(
        &self,
        key: &str,
        data: impl AsRef<[u8]>,
        content_type: &str,
    ) -> Result<ResponseData, OssError> {
        retry_429("put", key, || async {
            self.inner
                .bucket
                .put_object_with_content_type(key, data.as_ref(), content_type)
                .await
                .map_err(OssError::from)
        })
        .await
    }

    /// 以流式方式上传本地文件到 OSS；大文件自动走 multipart,429 重试时重新打开文件从头上传
    pub async fn upload_file(
        &self,
        key: &str,
        path: impl AsRef<Path>,
        content_type: &str,
    ) -> Result<(), OssError> {
        let path = path.as_ref().to_owned();
        retry_429("put_stream", key, || {
            let path = path.clone();
            async move {
                let mut file = tokio::fs::File::open(path).await?;
                self.inner
                    .bucket
                    .put_object_stream_with_content_type(&mut file, key, content_type)
                    .await
                    .map(|_| ())
                    .map_err(OssError::from)
            }
        })
        .await
    }

    /// 删除单个文件
    pub async fn delete(&self, key: &str) -> Result<ResponseData, OssError> {
        retry_429("delete", key, || async {
            self.inner
                .bucket
                .delete_object(key)
                .await
                .map_err(OssError::from)
        })
        .await
    }

    /// 批量删除文件；分片并发执行,遇错即停
    pub async fn delete_batch(&self, keys: Vec<impl AsRef<str>>) -> Result<(), OssError> {
        async {
            for concurrent_chunks in keys.chunks(CHUNK_SIZE * CONCURRENCY) {
                let futures: Vec<_> = concurrent_chunks
                    .chunks(CHUNK_SIZE)
                    .map(|chunk| async move {
                        for key in chunk {
                            retry_429("delete_batch", key.as_ref(), || async {
                                self.inner
                                    .bucket
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
        .await
    }

    /// 获取对象存储端点(创建时的 `endpoint` 配置值)
    pub fn endpoint(&self) -> &str {
        &self.inner.endpoint
    }

    /// 获取文件的公开访问 URL
    pub fn get_url(&self, key: &str) -> String {
        format!("{}/{}", self.inner.public_url, key.trim_start_matches('/'))
    }

    /// 获取文件的签名 URL(无图片处理参数)
    pub async fn get_signed_url(&self, key: &str, expires: Duration) -> Result<String, OssError> {
        self.get_signed_url_with_params(key, expires, None).await
    }

    /// 获取带图片处理参数的签名 URL
    pub async fn get_signed_url_with_params(
        &self,
        key: &str,
        expires: Duration,
        process: Option<String>,
    ) -> Result<String, OssError> {
        #[cfg(feature = "metrics")]
        let _timer = common_metrics::MetricsTimer::start(metric_name("sign", "duration_seconds"));
        #[cfg(feature = "metrics")]
        common_metrics::metrics::counter!(metric_name("sign", "requests")).increment(1);

        let custom_queries = if let Some(p) = process {
            let mut queries = HashMap::new();
            queries.insert("x-oss-process".to_string(), p);
            Some(queries)
        } else {
            None
        };

        match self
            .inner
            .bucket
            .presign_get(key, expires.as_secs() as u32, custom_queries)
            .await
        {
            Ok(url) => Ok(url),
            Err(error) => {
                #[cfg(feature = "metrics")]
                common_metrics::metrics::counter!(metric_name("sign", "errors")).increment(1);
                Err(OssError::from(error))
            }
        }
    }

    /// 下载文件
    pub async fn download(&self, key: &str) -> Result<Bytes, OssError> {
        async {
            let response_data = retry_429("get", key, || async {
                self.inner
                    .bucket
                    .get_object(key)
                    .await
                    .map_err(OssError::from)
            })
            .await?;

            Ok::<Bytes, OssError>(response_data.into_bytes())
        }
        .await
    }

    /// 判断对象是否存在；以 list(前缀 = key) 判定,不触发底层对 404 的重试与告警
    pub async fn exists(&self, key: &str) -> Result<bool, OssError> {
        #[cfg(feature = "metrics")]
        let _timer = common_metrics::MetricsTimer::start(metric_name("exists", "duration_seconds"));
        #[cfg(feature = "metrics")]
        common_metrics::metrics::counter!(metric_name("exists", "requests")).increment(1);

        let key = key.trim_start_matches('/');
        let results = match self
            .inner
            .bucket
            .list(key.to_string(), None)
            .await
            .map_err(OssError::from)
        {
            Ok(results) => results,
            Err(error) => {
                #[cfg(feature = "metrics")]
                common_metrics::metrics::counter!(metric_name("exists", "errors")).increment(1);
                return Err(error);
            }
        };

        Ok(results
            .iter()
            .flat_map(|result| result.contents.iter())
            .any(|object| object.key.as_str() == key))
    }

    /// 列出指定前缀下的全部对象 key(底层自动处理分页)
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>, OssError> {
        retry_429("list", prefix, || async {
            self.inner
                .bucket
                .list(prefix.to_string(), None)
                .await
                .map_err(OssError::from)
        })
        .await
        .map(|results| {
            results
                .into_iter()
                .flat_map(|result| result.contents.into_iter())
                .map(|object| object.key)
                .collect()
        })
    }

    /// 获取对象存储的流式下载响应
    pub async fn get_download_stream_response(
        &self,
        key: &str,
    ) -> Result<impl Stream<Item = Result<Bytes, OssError>> + use<>, OssError> {
        async {
            let response = retry_429("get_stream", key, || async {
                self.inner
                    .bucket
                    .get_object_stream(key)
                    .await
                    .map_err(OssError::from)
            })
            .await?;
            Ok::<_, OssError>(response.bytes.map(|item| item.map_err(OssError::from)))
        }
        .await
    }

    /// 获取对象存储的流式下载响应, 并透传单段 `Range` 头(如 `bytes=0-499`);
    /// `range` 为 `None` 时读取整个对象。
    ///
    /// 通过预签名 URL + reqwest 发起一次请求: 既保持响应体流式, 又能读到
    /// `Content-Range` / `Content-Length`, 供上层构造 206 / 416 响应。
    pub async fn get_download_stream_range_response(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<DownloadStream, OssError> {
        let url = self.get_signed_url(key, Duration::from_secs(3600)).await?;
        let range = range.map(str::to_owned);

        let response = retry_429("get_stream_range", key, || {
            let url = url.clone();
            let range = range.clone();
            async move {
                let mut request = reqwest::Client::new().get(&url);
                if let Some(range) = range.as_deref() {
                    request = request.header(reqwest::header::RANGE, range);
                }
                let response = request.send().await?;
                // 仅 429 交给退避重试; 416 是合法的范围不足响应, 需要原样返回
                if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    return Err(OssError::from_response(response).await);
                }
                Ok(response)
            }
        })
        .await?;

        // 非 2xx 除 416 外都视为存储错误, 消费响应体后返回完整错误
        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::RANGE_NOT_SATISFIABLE
        {
            return Err(OssError::from_response(response).await);
        }

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_length = response.content_length();
        let stream = response
            .bytes_stream()
            .map(|item| item.map_err(OssError::from));

        Ok(DownloadStream {
            status,
            content_type,
            content_range,
            content_length,
            stream: Box::pin(stream),
        })
    }

    /// 下载对象并应用指定的图片处理参数
    pub async fn download_with_process(&self, key: &str, process: &str) -> Result<Bytes, OssError> {
        let url = self
            .get_signed_url_with_params(key, Duration::from_secs(3600), Some(process.to_string()))
            .await?;
        retry_429("get_with_process", key, || async {
            let response = reqwest::get(&url).await?;
            if !response.status().is_success() {
                return Err(OssError::from_response(response).await);
            }

            response.bytes().await.map_err(OssError::from)
        })
        .await
    }

    /// 获取应用图片处理参数后的流式下载响应
    pub async fn download_stream_with_process(
        &self,
        key: &str,
        process: &str,
    ) -> Result<impl Stream<Item = Result<Bytes, OssError>> + use<>, OssError> {
        let url = self
            .get_signed_url_with_params(key, Duration::from_secs(3600), Some(process.to_string()))
            .await?;

        async {
            let response = retry_429("stream_with_process", key, || async {
                let response = reqwest::get(&url).await?;
                if !response.status().is_success() {
                    return Err(OssError::from_response(response).await);
                }
                Ok(response)
            })
            .await?;

            Ok::<_, OssError>(response.bytes_stream().map(|r| r.map_err(OssError::from)))
        }
        .await
    }
}
