use bytes::Bytes;
use reqwest::StatusCode;
use thiserror::Error;

use common_core::error::{AppError, ContextualError};

/// OSS / S3 / 其他 HTTP 对象存储服务的统一错误类型
///
/// 相比 `rust-s3` 自带的 [`s3::error::S3Error`]，新增 [`OssError::Http`] 变体：
/// 存储服务返回 4xx/5xx 时携带完整的 status + headers + body，日志里可直接看到完整响应。
#[derive(Debug, Error)]
pub enum OssError {
    /// 本地文件操作失败
    #[error("文件操作失败: {0}")]
    Io(#[from] std::io::Error),

    /// 底层 `rust-s3` 库错误（网络、签名、XML 解析等）
    #[error("s3 客户端错误: {0}")]
    Inner(#[from] s3::error::S3Error),

    /// reqwest 层错误（连接失败、超时等，拿不到 HTTP 响应）
    #[error("请求错误: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// 收到 HTTP 非 2xx 响应，携带完整响应信息
    ///
    /// headers 不进 Display（避免在错误消息里泄漏敏感头），日志走 Debug 时可见。
    #[error("HTTP {status} {url} body: {}", String::from_utf8_lossy(.body))]
    Http {
        url: String,
        status: StatusCode,
        body: Bytes,
    },
}

impl OssError {
    /// 判断错误是否为 OSS 限流(HTTP 429 Too Many Requests)；命中后可按 `RETRY_DELAYS` 退避重试
    pub fn is_rate_limited(&self) -> bool {
        match self {
            OssError::Http { status, .. } => status.as_u16() == 429,
            OssError::Inner(s3::error::S3Error::HttpFailWithBody(status, _)) => *status == 429,
            _ => false,
        }
    }

    /// 判断错误是否为对象不存在(HTTP 404)
    pub fn is_not_found(&self) -> bool {
        match self {
            OssError::Http { status, .. } => *status == StatusCode::NOT_FOUND,
            OssError::Inner(s3::error::S3Error::HttpFailWithBody(status, _)) => *status == 404,
            _ => false,
        }
    }

    /// 消费一个非 2xx 的响应，把 status / headers / body 完整装进 [`OssError::Http`]
    pub async fn from_response(response: reqwest::Response) -> Self {
        let url = response.url().to_string();
        let status = response.status();
        let body = response.bytes().await.unwrap_or_default();
        Self::Http { url, status, body }
    }
}

impl From<OssError> for ContextualError {
    /// 将 OSS 错误转换为上下文错误
    fn from(value: OssError) -> Self {
        ContextualError::error("oss_error", "Oss错误", value, AppError::InternalServerError)
    }
}
