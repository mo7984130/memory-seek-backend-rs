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
    pub fn new(base_url: &str, auth_client: Arc<AuthClient>) -> Self {
        Self {
            http: reqwest::Client::new(),
            auth_client,
            base_url: base_url.to_string(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn auth_client(&self) -> &Arc<AuthClient> {
        &self.auth_client
    }

    pub fn raw(&self) -> &reqwest::Client {
        &self.http
    }

    /// 获取用户信息
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

    /// 生成邀请码
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

    /// 修改昵称
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

    /// 修改密码
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

    /// 登出
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

    /// 批量获取用户信息
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
    pub async fn get_user_info_batch_concurrent(
        &self,
        user_ids: &[i64],
        concurrency: usize,
    ) -> Vec<Option<UserInfoVO>> {
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