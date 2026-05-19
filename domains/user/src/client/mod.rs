use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;

use auth::client::AuthClient;

use crate::models::{ChangeNicknameRequest, ChangePasswordRequest, GetUserInfoBatchRequest};

#[derive(Deserialize)]
#[allow(dead_code)]
struct ApiResponse<T> {
    code: u16,
    msg: Option<String>,
    data: Option<T>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserDTO {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub email: String,
    pub avatar_token: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InviterCodeDTO {
    pub inviter_code: String,
    pub expire_at: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoVO {
    pub user_id: String,
    pub nickname: String,
    pub avatar_token: Option<String>,
}

pub struct UserClient {
    http: reqwest::Client,
    auth_client: Arc<AuthClient>,
    base_url: String,
}

impl UserClient {
    /// 创建新的 `UserClient` 实例
    ///
    /// # 参数
    /// - `base_url`: 用户服务的基础 URL
    /// - `auth_client`: 用于认证的 `AuthClient` 实例
    pub fn new(base_url: &str, auth_client: Arc<AuthClient>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            auth_client,
            base_url: base_url.to_string(),
        }
    }

    /// 返回用户服务的基础 URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 返回认证客户端的引用
    pub fn auth_client(&self) -> &Arc<AuthClient> {
        &self.auth_client
    }

    /// 返回底层 reqwest HTTP 客户端的引用
    pub fn raw(&self) -> &reqwest::Client {
        &self.http
    }

    /// 根据用户 ID 获取单个用户信息
    ///
    /// # 参数
    /// - `user_id`: 目标用户的 ID
    ///
    /// # 返回
    /// 返回包含用户详细信息的 `UserDTO`
    ///
    /// # 错误
    /// - `anyhow::Error`: HTTP 请求失败、响应解析失败或 API 返回错误时
    pub async fn get_user_info(&self, user_id: i64) -> Result<UserDTO> {
        let resp = self
            .auth_client
            .get("/user/info", user_id)
            .await?
            .send()
            .await
            .context("get user info request failed")?;

        let status = resp.status();
        let api_resp: ApiResponse<UserDTO> = resp.json().await.context("get user info parse failed")?;
        let msg = api_resp.msg.unwrap_or_default();
        let user = api_resp
            .data
            .context(format!("get user info failed (status={}): {}", status, msg))?;

        Ok(user)
    }

    /// 为指定用户生成邀请码
    ///
    /// # 参数
    /// - `user_id`: 需要生成邀请码的用户 ID
    ///
    /// # 返回
    /// 返回包含邀请码及过期时间的 `InviterCodeDTO`
    ///
    /// # 错误
    /// - `anyhow::Error`: HTTP 请求失败、响应解析失败或 API 返回错误时
    pub async fn generate_inviter_code(&self, user_id: i64) -> Result<InviterCodeDTO> {
        let resp = self
            .auth_client
            .get("/user/inviter-code", user_id)
            .await?
            .send()
            .await
            .context("generate inviter code request failed")?;

        let status = resp.status();
        let api_resp: ApiResponse<InviterCodeDTO> = resp.json().await.context("generate inviter code parse failed")?;
        let msg = api_resp.msg.unwrap_or_default();
        let code = api_resp
            .data
            .context(format!("generate inviter code failed (status={}): {}", status, msg))?;

        Ok(code)
    }

    /// 修改指定用户的昵称
    ///
    /// # 参数
    /// - `user_id`: 需要修改昵称的用户 ID
    /// - `new_nickname`: 新的昵称
    ///
    /// # 返回
    /// 返回修改后的昵称
    ///
    /// # 错误
    /// - `anyhow::Error`: HTTP 请求失败、响应解析失败或 API 返回错误时
    pub async fn change_nickname(&self, user_id: i64, new_nickname: &str) -> Result<String> {
        let req = ChangeNicknameRequest {
            new_nickname: new_nickname.to_string(),
        };
        let resp = self
            .auth_client
            .post("/user/nickname", user_id)
            .await?
            .json(&req)
            .send()
            .await
            .context("change nickname request failed")?;

        let status = resp.status();
        let api_resp: ApiResponse<String> = resp.json().await.context("change nickname parse failed")?;
        let msg = api_resp.msg.unwrap_or_default();
        let nickname = api_resp
            .data
            .context(format!("change nickname failed (status={}): {}", status, msg))?;

        Ok(nickname)
    }

    /// 修改指定用户的密码
    ///
    /// # 参数
    /// - `user_id`: 需要修改密码的用户 ID
    /// - `old_password`: 旧密码
    /// - `new_password`: 新密码
    ///
    /// # 错误
    /// - `anyhow::Error`: HTTP 请求失败、响应解析失败或旧密码不正确时
    pub async fn change_password(&self, user_id: i64, old_password: &str, new_password: &str) -> Result<()> {
        let req = ChangePasswordRequest {
            old_password: old_password.to_string(),
            new_password: new_password.to_string(),
        };
        let resp = self
            .auth_client
            .post("/user/password", user_id)
            .await?
            .json(&req)
            .send()
            .await
            .context("change password request failed")?;

        let status = resp.status();
        let api_resp: ApiResponse<()> = resp.json().await.context("change password parse failed")?;
        let msg = api_resp.msg.unwrap_or_default();

        if api_resp.code != 200 {
            anyhow::bail!("change password failed (status={}): {}", status, msg);
        }

        Ok(())
    }

    /// 登出指定用户
    ///
    /// # 参数
    /// - `user_id`: 需要登出的用户 ID
    ///
    /// # 错误
    /// - `anyhow::Error`: HTTP 请求失败、响应解析失败或 API 返回错误时
    pub async fn logout(&self, user_id: i64) -> Result<()> {
        let resp = self
            .auth_client
            .post("/user/logout", user_id)
            .await?
            .send()
            .await
            .context("logout request failed")?;

        let status = resp.status();
        let api_resp: ApiResponse<()> = resp.json().await.context("logout parse failed")?;
        let msg = api_resp.msg.unwrap_or_default();

        if api_resp.code != 200 {
            anyhow::bail!("logout failed (status={}): {}", status, msg);
        }

        Ok(())
    }

    /// 批量获取多个用户的信息
    ///
    /// # 参数
    /// - `user_ids`: 需要查询的用户 ID 切片
    ///
    /// # 返回
    /// 返回 `Vec<Option<UserInfoVO>>`，每个元素对应一个用户的信息，找不到时为 `None`
    ///
    /// # 错误
    /// - `anyhow::Error`: `user_ids` 为空、HTTP 请求失败或响应解析失败时
    pub async fn get_user_info_batch(&self, user_ids: &[i64]) -> Result<Vec<Option<UserInfoVO>>> {
        let req = GetUserInfoBatchRequest {
            user_ids: user_ids.iter().map(|id| id.to_string()).collect(),
        };

        // 使用第一个用户ID进行认证，如果没有用户ID则使用公共接口
        let resp = if let Some(&first_id) = user_ids.first() {
            self.auth_client
                .post("/user/info/batch", first_id)
                .await?
                .json(&req)
                .send()
                .await
                .context("get user info batch request failed")?
        } else {
            anyhow::bail!("user_ids cannot be empty");
        };

        let status = resp.status();
        let api_resp: ApiResponse<Vec<Option<UserInfoVO>>> = resp.json().await.context("get user info batch parse failed")?;
        let msg = api_resp.msg.unwrap_or_default();
        let users = api_resp
            .data
            .context(format!("get user info batch failed (status={}): {}", status, msg))?;

        Ok(users)
    }

    /// 并发获取多个用户信息
    ///
    /// 优先使用批量接口，失败时自动回退到逐个请求。
    ///
    /// # 参数
    /// - `user_ids`: 需要查询的用户 ID 切片
    /// - `concurrency`: 回退到逐个请求时的最大并发数
    ///
    /// # 返回
    /// 返回 `Vec<Option<UserInfoVO>>`，每个元素对应一个用户的信息，失败时对应位置为 `None`
    pub async fn get_user_info_batch_concurrent(
        &self,
        user_ids: &[i64],
        concurrency: usize,
    ) -> Vec<Option<UserInfoVO>> {
        match self.get_user_info_batch(user_ids).await {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!(error = %e, "批量获取用户信息失败，回退到逐个请求");
                use futures::stream::{self, StreamExt};
                let results: Vec<Option<UserDTO>> = stream::iter(user_ids.iter())
                    .map(|&user_id| async move { self.get_user_info(user_id).await.ok() })
                    .buffer_unordered(concurrency)
                    .collect()
                    .await;

                results.into_iter().map(|opt| opt.map(|user| UserInfoVO {
                    user_id: user.id,
                    nickname: user.nickname,
                    avatar_token: user.avatar_token,
                })).collect()
            }
        }
    }
}
