mod token_store;

pub use token_store::TokenStore;

use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Method;
use validator::Validate;

use crate::models::{RegisterRequest, SendEmailCodeRequest};

pub struct AuthClient {
    http: reqwest::Client,
    token_store: Arc<TokenStore>,
    base_url: String,
}

impl AuthClient {
    pub fn new(base_url: &str, token_store: Arc<TokenStore>) -> Self {
        Self {
            http: reqwest::Client::new(),
            token_store,
            base_url: base_url.to_string(),
        }
    }

    pub async fn request(&self, method: Method, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        let (uid, token) = self.token_store.get_auth(user_id).await?;
        Ok(self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header("Authorization", format!("{} {}", uid, token)))
    }

    pub async fn get(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::GET, path, user_id).await
    }

    pub async fn post(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::POST, path, user_id).await
    }

    pub async fn put(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::PUT, path, user_id).await
    }

    pub async fn patch(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::PATCH, path, user_id).await
    }

    pub async fn delete(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::DELETE, path, user_id).await
    }

    /// 发送不带认证头的请求（用于登录、注册等公开接口）
    pub fn public_request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.http.request(method, format!("{}{}", self.base_url, path))
    }

    pub fn public_get(&self, path: &str) -> reqwest::RequestBuilder {
        self.public_request(Method::GET, path)
    }

    pub fn public_post(&self, path: &str) -> reqwest::RequestBuilder {
        self.public_request(Method::POST, path)
    }

    pub fn raw(&self) -> &reqwest::Client {
        &self.http
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn token_store(&self) -> &Arc<TokenStore> {
        &self.token_store
    }

    /// 登录并返回用户ID
    pub async fn login(&self, account: &str, password: &str) -> Result<i64> {
        self.token_store.login(account, password).await
    }

    /// 注册新用户并返回用户ID
    pub async fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
        nickname: &str,
        inviter_code: &str,
        email_verify_code: &str,
    ) -> Result<i64> {
        let req = RegisterRequest {
            username: username.to_string(),
            email: email.to_string(),
            password: password.to_string(),
            nickname: nickname.to_string(),
            inviter_code: inviter_code.to_string(),
            email_verify_code: email_verify_code.to_string(),
        };
        req.validate()
            .context("register request validation failed")?;

        let resp = self
            .public_post("/auth/register")
            .json(&req)
            .send()
            .await
            .context("register request failed")?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await.context("register parse failed")?;
        let msg = body["msg"].as_str().unwrap_or("");
        let code = body["code"].as_u64().unwrap_or(0);

        if code != 200 {
            anyhow::bail!("register failed (status={}): {}", status, msg);
        }

        let user_id = body["data"]["id"]
            .as_str()
            .context("missing user id")?
            .parse::<i64>()
            .context("invalid user id")?;

        Ok(user_id)
    }

    /// 发送邮箱验证码
    pub async fn send_email_code(&self, email: &str) -> Result<()> {
        let req = SendEmailCodeRequest {
            email: email.to_string(),
        };
        req.validate()
            .context("send email code request validation failed")?;

        let resp = self
            .public_post("/auth/email-verify-code")
            .json(&req)
            .send()
            .await
            .context("send email code request failed")?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await.context("send email code parse failed")?;
        let msg = body["msg"].as_str().unwrap_or("");
        let code = body["code"].as_u64().unwrap_or(0);

        if code != 200 {
            anyhow::bail!("send email code failed (status={}): {}", status, msg);
        }

        Ok(())
    }

    /// 刷新access token
    pub async fn refresh_access_token(
        &self,
        user_id: i64,
        refresh_token: &str,
    ) -> Result<(String, DateTime<Utc>)> {
        let resp = self
            .public_get("/auth/access-token")
            .header("x-user-id", user_id.to_string())
            .header("x-refresh-token", refresh_token)
            .send()
            .await
            .context("refresh access token request failed")?;

        let status = resp.status();
        let body: serde_json::Value =
            resp.json().await.context("refresh access token parse failed")?;
        let msg = body["msg"].as_str().unwrap_or("");
        let code = body["code"].as_u64().unwrap_or(0);

        if code != 200 {
            anyhow::bail!("refresh access token failed (status={}): {}", status, msg);
        }

        let access_token = body["data"]["accessToken"]
            .as_str()
            .context("missing access_token")?
            .to_string();

        let expire_at_str = body["data"]["accessTokenExpireAt"]
            .as_str()
            .context("missing access_token_expire_at")?;

        let expire_at: DateTime<Utc> = expire_at_str
            .parse()
            .context("invalid access_token_expire_at format")?;

        Ok((access_token, expire_at))
    }

    /// 批量登录多个用户并返回用户ID列表
    pub async fn batch_login(&self, credentials: &[(&str, &str)]) -> Result<Vec<i64>> {
        self.token_store.warmup(credentials).await
    }
}
