use std::time::Duration;

use base64::Engine;
use deadpool_redis::redis::AsyncCommands;
use memseek_test::ctxlibs;
use sea_orm::DatabaseConnection;
use serde::Deserialize;

pub struct Context {
    pub client: ctxlibs::http_client::Client,
    pub db: DatabaseConnection,
    pub mailhog: ctxlibs::http_client::Client,
    pub redis: deadpool_redis::Pool,
    pub s3: oss::S3Client,
}

pub use ctxlibs::http_client::{HttpError, reqwest};

// MailHog API v2 响应结构(字段为 PascalCase, 仅取需要的字段)
#[derive(Debug, Deserialize)]
struct MailHogMessages {
    items: Vec<MailHogMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MailHogMessage {
    to: Vec<MailHogAddress>,
    content: MailHogContent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MailHogAddress {
    mailbox: String,
    domain: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MailHogContent {
    body: String,
}

impl Context {
    fn mailhog_not_found(&self) -> HttpError {
        HttpError::status(
            reqwest::StatusCode::NOT_FOUND,
            reqwest::Method::GET,
            &self.mailhog.base_url,
        )
    }

    /// 查询 MailHog 中发给指定邮箱的最新一封邮件(按时间倒序), 提取正文验证码。
    /// 未找到时返回 `HttpError::Status`(404)。
    pub async fn mailhog_latest_code(&self, email: &str) -> Result<String, HttpError> {
        let messages = self
            .mailhog
            .request(
                reqwest::Method::GET,
                "/api/v2/messages?limit=100&order=desc",
            )
            .send_checked()
            .await?
            .json::<MailHogMessages>()
            .await?;
        let body_raw = &messages
            .items
            .iter()
            .find(|m| {
                m.to.iter()
                    .any(|t| format!("{}@{}", t.mailbox, t.domain) == email)
            })
            .ok_or_else(|| self.mailhog_not_found())?
            .content
            .body;
        // MailHog 的 Content.Body 为 MIME base64(76 字符换行), 解码前需去除空白
        let cleaned: String = body_raw.chars().filter(|c| !c.is_whitespace()).collect();
        let body = base64::engine::general_purpose::STANDARD
            .decode(cleaned)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| self.mailhog_not_found())?;
        extract_code(&body).ok_or_else(|| self.mailhog_not_found())
    }

    /// 轮询 MailHog 直到取到验证码(邮件异步投递)。
    ///
    /// 只有当邮件里的验证码与 server 实际存储(Redis)的一致时才返回:
    /// server 每次发码都会先在 Redis 写入新验证码(先于邮件投递),
    /// 因此该一致性证明邮件是本次发送的最新一封, 避免历史邮件中的旧验证码。
    /// 超时(约 4s)返回 404。
    pub async fn wait_mailhog_code(&self, email: &str) -> Result<String, HttpError> {
        for _ in 0..20 {
            if let Ok(code) = self.mailhog_latest_code(email).await
                && self.redis_email_code(email).await.as_deref() == Some(code.as_str())
            {
                return Ok(code);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err(self.mailhog_not_found())
    }

    /// 读取 server 实际存储到 Redis 的邮箱验证码(与邮件中的比对)。
    pub async fn redis_email_code(&self, email: &str) -> Option<String> {
        let mut conn = self.redis.get().await.ok()?;
        let key = constants::redis_keys::auth::email_verify_code(email);
        conn.get(&key).await.ok().flatten()
    }

    /// 读取 Redis 字符串值(不存在或连接失败返回 `None`)。
    pub async fn redis_get(&self, key: &str) -> Option<String> {
        let mut conn = self.redis.get().await.ok()?;
        conn.get(key).await.ok().flatten()
    }

    /// 读取 Redis key 剩余 TTL(秒; `-1` 无过期, `-2` 不存在)。
    pub async fn redis_ttl(&self, key: &str) -> Option<i64> {
        let mut conn = self.redis.get().await.ok()?;
        conn.ttl(key).await.ok()
    }

    /// 下载 S3 对象内容; 不存在或请求失败返回 `None`。
    pub async fn s3_bytes(&self, key: &str) -> Option<Vec<u8>> {
        self.s3.download(key).await.ok().map(|b| b.to_vec())
    }

    /// 对象确实不存在; 用 list 判定, 避免 GET/HEAD 对 404 的重试与告警。
    pub async fn s3_missing(&self, key: &str) -> bool {
        matches!(self.s3.exists(key).await, Ok(false))
    }
}

/// 从邮件 HTML 正文提取验证码: 正文含 `<strong>XXXXXX</strong>`。
fn extract_code(body: &str) -> Option<String> {
    let start = body.find("<strong>")? + "<strong>".len();
    let rest = &body[start..];
    let end = rest.find("</strong>")?;
    let code = &rest[..end];
    (code.len() == 6).then(|| code.to_string())
}
