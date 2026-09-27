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
    /// MailHog 内部 id, 用于删单封邮件(字段名为全大写 `ID`)
    #[serde(rename = "ID")]
    id: Option<String>,
    to: Vec<MailHogAddress>,
    content: MailHogContent,
}

impl MailHogMessage {
    /// 收件人是否包含该地址
    fn is_to(&self, email: &str) -> bool {
        self.to
            .iter()
            .any(|t| format!("{}@{}", t.mailbox, t.domain) == email)
    }

    /// 从邮件 HTML 正文提取验证码
    fn code(&self) -> Option<String> {
        // MailHog 的 Content.Body 为 MIME base64(76 字符换行), 解码前需去除空白
        let cleaned: String = self
            .content
            .body
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let body = base64::engine::general_purpose::STANDARD
            .decode(cleaned)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())?;
        extract_code(&body)
    }
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
        self.mailhog_messages(email)
            .await?
            .iter()
            .find_map(MailHogMessage::code)
            .ok_or_else(|| self.mailhog_not_found())
    }

    /// 删除发给指定邮箱的全部邮件(收尾用), 返回删除封数。
    ///
    /// 只删该收件人的邮件: 压测并发下其它场景可能正在等自己那封验证码,
    /// 不能使用 MailHog 的"删除全部"接口。
    pub async fn mailhog_purge(&self, email: &str) -> Result<usize, HttpError> {
        let messages = self.mailhog_messages(email).await?;
        let mut deleted = 0;
        for id in messages.iter().filter_map(|m| m.id.as_deref()) {
            let path = format!("/api/v1/messages/{id}");
            let response = self
                .mailhog
                .request(reqwest::Method::DELETE, &path)
                .send()
                .await?;
            let status = response.status();
            if status.is_success() || status == reqwest::StatusCode::NOT_FOUND {
                deleted += 1;
            } else {
                tracing::warn!(%status, %id, "删除邮件失败(忽略)");
            }
        }
        Ok(deleted)
    }

    /// 按收件人查询邮件(新的在前)。
    ///
    /// 优先走 v2 search 的服务端过滤: 压测下每秒上百封邮件时,
    /// 全局 `limit=100` 的窗口会被其它任务的邮件挤出, 造成偶发"找不到邮件"。
    /// search 不可用时退回全局列表 + 本地过滤。
    async fn mailhog_messages(&self, email: &str) -> Result<Vec<MailHogMessage>, HttpError> {
        const SEARCH_LIMIT: &str = "100";

        let searched = self
            .mailhog
            .request(reqwest::Method::GET, "/api/v2/search")
            .query("kind", "to")
            .query("query", email)
            .query("limit", SEARCH_LIMIT)
            .query("order", "desc")
            .send()
            .await;

        let items = match searched {
            Ok(response) if response.status().is_success() => {
                response.json::<MailHogMessages>().await?.items
            }
            _ => {
                tracing::warn!("MailHog search 不可用, 退回全局列表过滤收件人");
                self.mailhog
                    .request(
                        reqwest::Method::GET,
                        "/api/v2/messages?limit=100&order=desc",
                    )
                    .send_checked()
                    .await?
                    .json::<MailHogMessages>()
                    .await?
                    .items
            }
        };
        // search 命中与本地过滤都做一遍: 万一声明式过滤被忽略也不会串到别人的邮件
        Ok(items
            .into_iter()
            .filter(|message| message.is_to(email))
            .collect())
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

    /// 删除 Redis key(收尾用), 返回是否删除成功。
    pub async fn redis_del(&self, key: &str) -> bool {
        let Ok(mut conn) = self.redis.get().await else {
            return false;
        };
        let deleted: Result<u64, _> = conn.del(key).await;
        deleted.is_ok()
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
