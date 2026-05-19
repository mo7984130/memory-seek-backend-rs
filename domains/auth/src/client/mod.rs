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
    /// 创建新的认证客户端实例
    ///
    /// # 参数
    /// - `base_url`: 认证服务的基础 URL
    /// - `token_store`: 共享的 token 存储
    pub fn new(base_url: &str, token_store: Arc<TokenStore>) -> Self {
        Self {
            http: reqwest::Client::new(),
            token_store,
            base_url: base_url.to_string(),
        }
    }

    /// 创建带认证头的 HTTP 请求构建器
    ///
    /// # 参数
    /// - `method`: HTTP 方法
    /// - `path`: 请求路径（相对于 base_url）
    /// - `user_id`: 用户 ID，用于查找对应 token
    ///
    /// # 返回
    /// 附加了 Authorization 头的请求构建器
    ///
    /// # 错误
    /// - `anyhow::Error`: token 获取失败或用户未登录
    pub async fn request(&self, method: Method, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        let (uid, token) = self.token_store.get_auth(user_id).await?;
        Ok(self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header("Authorization", format!("{} {}", uid, token)))
    }

    /// 创建带认证头的 GET 请求
    ///
    /// # 参数
    /// - `path`: 请求路径（相对于 base_url）
    /// - `user_id`: 用户 ID
    ///
    /// # 返回
    /// GET 方法的请求构建器
    ///
    /// # 错误
    /// - `anyhow::Error`: token 获取失败
    pub async fn get(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::GET, path, user_id).await
    }

    /// 创建带认证头的 POST 请求
    ///
    /// # 参数
    /// - `path`: 请求路径（相对于 base_url）
    /// - `user_id`: 用户 ID
    ///
    /// # 返回
    /// POST 方法的请求构建器
    ///
    /// # 错误
    /// - `anyhow::Error`: token 获取失败
    pub async fn post(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::POST, path, user_id).await
    }

    /// 创建带认证头的 PUT 请求
    ///
    /// # 参数
    /// - `path`: 请求路径（相对于 base_url）
    /// - `user_id`: 用户 ID
    ///
    /// # 返回
    /// PUT 方法的请求构建器
    ///
    /// # 错误
    /// - `anyhow::Error`: token 获取失败
    pub async fn put(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::PUT, path, user_id).await
    }

    /// 创建带认证头的 PATCH 请求
    ///
    /// # 参数
    /// - `path`: 请求路径（相对于 base_url）
    /// - `user_id`: 用户 ID
    ///
    /// # 返回
    /// PATCH 方法的请求构建器
    ///
    /// # 错误
    /// - `anyhow::Error`: token 获取失败
    pub async fn patch(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::PATCH, path, user_id).await
    }

    /// 创建带认证头的 DELETE 请求
    ///
    /// # 参数
    /// - `path`: 请求路径（相对于 base_url）
    /// - `user_id`: 用户 ID
    ///
    /// # 返回
    /// DELETE 方法的请求构建器
    ///
    /// # 错误
    /// - `anyhow::Error`: token 获取失败
    pub async fn delete(&self, path: &str, user_id: i64) -> Result<reqwest::RequestBuilder> {
        self.request(Method::DELETE, path, user_id).await
    }

    /// 创建不带认证头的请求构建器，用于登录、注册等公开接口
    ///
    /// # 参数
    /// - `method`: HTTP 方法
    /// - `path`: 请求路径（相对于 base_url）
    ///
    /// # 返回
    /// 不含 Authorization 头的请求构建器
    pub fn public_request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.http.request(method, format!("{}{}", self.base_url, path))
    }

    /// 创建不带认证头的 GET 请求
    ///
    /// # 参数
    /// - `path`: 请求路径（相对于 base_url）
    ///
    /// # 返回
    /// 不含 Authorization 头的 GET 请求构建器
    pub fn public_get(&self, path: &str) -> reqwest::RequestBuilder {
        self.public_request(Method::GET, path)
    }

    /// 创建不带认证头的 POST 请求
    ///
    /// # 参数
    /// - `path`: 请求路径（相对于 base_url）
    ///
    /// # 返回
    /// 不含 Authorization 头的 POST 请求构建器
    pub fn public_post(&self, path: &str) -> reqwest::RequestBuilder {
        self.public_request(Method::POST, path)
    }

    /// 获取底层 reqwest HTTP 客户端的引用
    ///
    /// # 返回
    /// 内部 reqwest::Client 的不可变引用
    pub fn raw(&self) -> &reqwest::Client {
        &self.http
    }

    /// 获取认证服务的基础 URL
    ///
    /// # 返回
    /// base_url 字符串切片
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 获取共享 token 存储的引用
    ///
    /// # 返回
    /// Arc 包裹的 TokenStore 引用
    pub fn token_store(&self) -> &Arc<TokenStore> {
        &self.token_store
    }

    /// 登录用户并返回用户 ID，token 会被缓存到 TokenStore
    ///
    /// # 参数
    /// - `account`: 用户账号
    /// - `password`: 用户密码
    ///
    /// # 返回
    /// 登录成功的用户 ID
    ///
    /// # 错误
    /// - `anyhow::Error`: 登录请求失败或响应解析错误
    pub async fn login(&self, account: &str, password: &str) -> Result<i64> {
        self.token_store.login(account, password).await
    }

    /// 注册新用户并返回用户 ID
    ///
    /// # 参数
    /// - `username`: 用户名
    /// - `email`: 邮箱地址
    /// - `password`: 密码
    /// - `nickname`: 昵称
    /// - `inviter_code`: 邀请码
    /// - `email_verify_code`: 邮箱验证码
    ///
    /// # 返回
    /// 注册成功的用户 ID
    ///
    /// # 错误
    /// - `anyhow::Error`: 参数校验失败、请求失败或注册接口返回错误
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
    ///
    /// # 参数
    /// - `email`: 目标邮箱地址
    ///
    /// # 错误
    /// - `anyhow::Error`: 参数校验失败、请求失败或接口返回错误
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

    /// 使用 refresh_token 刷新 access_token
    ///
    /// # 参数
    /// - `user_id`: 用户 ID
    /// - `refresh_token`: 刷新令牌
    ///
    /// # 返回
    /// 元组 `(access_token, expire_at)`，包含新令牌和过期时间
    ///
    /// # 错误
    /// - `anyhow::Error`: 请求失败、refresh_token 无效或响应解析错误
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

    /// 批量登录多个用户并返回用户 ID 列表
    ///
    /// # 参数
    /// - `credentials`: 凭证列表，每个元素为 `(account, password)` 元组
    ///
    /// # 返回
    /// 登录成功的用户 ID 列表
    ///
    /// # 错误
    /// - `anyhow::Error`: 任意用户登录失败
    pub async fn batch_login(&self, credentials: &[(&str, &str)]) -> Result<Vec<i64>> {
        self.token_store.warmup(credentials).await
    }
}
