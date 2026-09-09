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
}

pub use ctxlibs::http_client::HttpError;

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
    /// 查询 MailHog 中发给指定邮箱的最新一封邮件, 提取正文验证码。
    /// 未找到时返回 `HttpError::Status(404)`。
    pub async fn mailhog_latest_code(&self, email: &str) -> Result<String, HttpError> {
        let messages = self
            .mailhog
            .get("/api/v2/messages")
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
            .ok_or(HttpError::Status(reqwest::StatusCode::NOT_FOUND))?
            .content
            .body;
        // MailHog 的 Content.Body 为 MIME base64(76 字符换行), 解码前需去除空白
        let cleaned: String = body_raw.chars().filter(|c| !c.is_whitespace()).collect();
        let body = base64::engine::general_purpose::STANDARD
            .decode(cleaned)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or(HttpError::Status(reqwest::StatusCode::NOT_FOUND))?;
        extract_code(&body).ok_or(HttpError::Status(reqwest::StatusCode::NOT_FOUND))
    }

    /// 轮询 MailHog 直到取到验证码(邮件异步投递), 超时(约 4s)返回 404。
    pub async fn wait_mailhog_code(&self, email: &str) -> Result<String, HttpError> {
        for _ in 0..20 {
            if let Ok(code) = self.mailhog_latest_code(email).await {
                return Ok(code);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err(HttpError::Status(reqwest::StatusCode::NOT_FOUND))
    }

    /// 读取 server 实际存储到 Redis 的邮箱验证码(与邮件中的比对)。
    pub async fn redis_email_code(&self, email: &str) -> Option<String> {
        let mut conn = self.redis.get().await.ok()?;
        let key = constants::redis_keys::auth::email_verify_code(email);
        conn.get(&key).await.ok().flatten()
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
