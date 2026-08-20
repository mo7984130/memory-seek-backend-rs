//! OSS 429 限流指数退避重试
//!
//! 阿里云 OSS 在突发流量 / QPS 超限时返回 `429 TooManyRequests`（错误码 `SlowDown`）。
//! 本模块提供仅针对 429 的退避重试，其余错误（网络、4xx 非 429、5xx）立即返回，
//! 避免掩盖真实失败原因。

use common::time::Duration;
use std::future::Future;

use crate::error::OssError;

/// 429 指数退避延迟表：第 1~4 次失败分别等待 100ms / 500ms / 2s / 5s
pub const RETRY_DELAYS: [Duration; 4] = [
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_secs(2),
    Duration::from_secs(5),
];

/// 对可能触发 OSS 限流的操作做指数退避重试
///
/// 仅对 `429 Too Many Requests` 重试，最多执行 1 + `delays.len()` 次尝试；
/// 第 `i` 次失败后等待 `delays[i]`（`i` 从 0 开始）再重试。
///
/// # 参数
/// - `op`: 操作名（`put` / `get` / `delete` / ...），仅用于指标标签
/// - `key`: 操作的 OSS 键名，仅用于日志定位
/// - `delays`: 各次失败对应的等待时长；生产代码传 [`RETRY_DELAYS`]，测试可注入更短的延迟
/// - `f`: 实际请求闭包
pub async fn retry_with_backoff<F, Fut, T>(
    op: &str,
    key: &str,
    delays: &[Duration],
    mut f: F,
) -> Result<T, OssError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, OssError>>,
{
    let _ = op;
    let mut attempts = 0usize;
    loop {
        match f().await {
            Ok(value) => return Ok(value),
            Err(err) if err.is_rate_limited() && attempts < delays.len() => {
                #[cfg(feature = "metrics")]
                metrics::counter!(format!("oss:{op}:retries")).increment(1);
                let delay = delays[attempts];
                tracing::debug!(
                    key = %key,
                    attempt = attempts + 1,
                    delay_ms = delay.as_millis(),
                    error = %err,
                    "OSS 429 限流，指数退避后重试"
                );
                tokio::time::sleep(delay).await;
                attempts += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

/// 使用默认延迟表 [`RETRY_DELAYS`] 的 429 重试，见 [`retry_with_backoff`]
pub async fn retry_429<F, Fut, T>(op: &str, key: &str, f: F) -> Result<T, OssError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, OssError>>,
{
    retry_with_backoff(op, key, &RETRY_DELAYS, f).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use reqwest::StatusCode;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn http_429() -> OssError {
        OssError::Http {
            url: "http://oss.test/key".into(),
            status: StatusCode::TOO_MANY_REQUESTS,
            body: Bytes::new(),
        }
    }

    fn inner_429() -> OssError {
        OssError::Inner(s3::error::S3Error::HttpFailWithBody(
            429,
            "SlowDown".to_string(),
        ))
    }

    fn http_500() -> OssError {
        OssError::Http {
            url: "http://oss.test/key".into(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: Bytes::new(),
        }
    }

    #[tokio::test]
    async fn retries_until_success() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let result = retry_with_backoff("test", "k", &[Duration::from_millis(1); 4], move || {
            let counter = Arc::clone(&counter);
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                if n < 3 { Err(http_429()) } else { Ok("ok") }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn gives_up_after_exhausting_delays() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let result = retry_with_backoff("test", "k", &[Duration::from_millis(1); 4], move || {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<(), OssError>(http_429())
            }
        })
        .await;

        assert!(matches!(
            result,
            Err(OssError::Http { status, .. }) if status == StatusCode::TOO_MANY_REQUESTS
        ));
        // 1 次原始请求 + 4 次重试
        assert_eq!(calls.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn retries_inner_s3_429() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let result = retry_with_backoff("test", "k", &[Duration::from_millis(1); 2], move || {
            let counter = Arc::clone(&counter);
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                if n < 2 { Err(inner_429()) } else { Ok(()) }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn does_not_retry_non_429() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let result = retry_with_backoff("test", "k", &[Duration::from_millis(1); 4], move || {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<(), OssError>(http_500())
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
