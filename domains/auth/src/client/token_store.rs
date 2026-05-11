use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use entities::user::UserDTO;
use tokio::sync::{Mutex, RwLock};

use crate::models::{AccessTokenResponse, LoginRequest};

#[allow(dead_code)]
struct TokenState {
    user_id: i64,
    account: String,
    access_token: String,
    access_token_expire_at: DateTime<Utc>,
    refresh_token: String,
    refresh_token_expire_at: DateTime<Utc>,
    refresh_lock: Mutex<()>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct TokenStateSnapshot {
    user_id: i64,
    account: String,
    access_token: String,
    access_token_expire_at: DateTime<Utc>,
    refresh_token: String,
    refresh_token_expire_at: DateTime<Utc>,
}

pub struct TokenStore {
    entries: RwLock<HashMap<i64, TokenState>>,
    base_url: String,
    http: reqwest::Client,
    refresh_before_expiry_secs: i64,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct ApiResponse<T> {
    code: u16,
    msg: Option<String>,
    data: Option<T>,
}

impl TokenState {
    fn snapshot(&self) -> TokenStateSnapshot {
        TokenStateSnapshot {
            user_id: self.user_id,
            account: self.account.clone(),
            access_token: self.access_token.clone(),
            access_token_expire_at: self.access_token_expire_at,
            refresh_token: self.refresh_token.clone(),
            refresh_token_expire_at: self.refresh_token_expire_at,
        }
    }
}

impl TokenStore {
    pub fn new(base_url: &str, refresh_before_expiry_secs: i64) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            base_url: base_url.to_string(),
            http: reqwest::Client::new(),
            refresh_before_expiry_secs,
        }
    }

    pub async fn login(&self, account: &str, password: &str) -> Result<i64> {
        let resp = self
            .http
            .post(format!("{}/auth/login", self.base_url))
            .json(&LoginRequest {
                account: account.to_string(),
                password: password.to_string(),
            })
            .send()
            .await
            .context("login request failed")?;

        let status = resp.status();
        let api_resp: ApiResponse<UserDTO> = resp.json().await.context("login parse failed")?;
        let msg = api_resp.msg.unwrap_or_default();
        let user = api_resp
            .data
            .context(format!("login failed (status={}): {}", status, msg))?;

        let user_id: i64 = user.id.parse().context("invalid user id")?;

        let state = TokenState {
            user_id,
            account: account.to_string(),
            access_token: user.access_token.clone().context("missing access_token")?,
            access_token_expire_at: user
                .access_token_expire_at
                .context("missing access_token_expire_at")?,
            refresh_token: user.refresh_token.clone().context("missing refresh_token")?,
            refresh_token_expire_at: user
                .refresh_token_expire_at
                .context("missing refresh_token_expire_at")?,
            refresh_lock: Mutex::new(()),
        };

        self.entries.write().await.insert(user_id, state);
        Ok(user_id)
    }

    /// 获取用户账号
    pub async fn get_account(&self, user_id: i64) -> Result<String> {
        let entries = self.entries.read().await;
        let state = entries.get(&user_id).context("user_id not found")?;
        Ok(state.account.clone())
    }

    /// 获取用户refresh_token
    pub async fn get_refresh_token(&self, user_id: i64) -> Result<String> {
        let entries = self.entries.read().await;
        let state = entries.get(&user_id).context("user_id not found")?;
        Ok(state.refresh_token.clone())
    }

    /// 检查用户是否已登录且token有效
    pub async fn is_token_valid(&self, user_id: i64) -> bool {
        let entries = self.entries.read().await;
        if let Some(state) = entries.get(&user_id) {
            let now = Utc::now();
            state.access_token_expire_at > now
        } else {
            false
        }
    }

    /// 移除用户的token状态
    pub async fn remove(&self, user_id: i64) -> Result<()> {
        self.entries.write().await.remove(&user_id);
        Ok(())
    }

    /// 获取所有已登录用户的ID列表
    pub async fn get_all_user_ids(&self) -> Vec<i64> {
        let entries = self.entries.read().await;
        entries.keys().copied().collect()
    }

    /// 清除所有token状态
    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }

    pub async fn get_auth(&self, user_id: i64) -> Result<(i64, String)> {
        // 快速路径：token未过期
        {
            let entries = self.entries.read().await;
            let state = entries.get(&user_id).context("user_id not found")?;

            let threshold = Utc::now() + Duration::seconds(self.refresh_before_expiry_secs);
            if state.access_token_expire_at > threshold {
                return Ok((user_id, state.access_token.clone()));
            }
        }

        // 慢路径：需要刷新token
        let snapshot = {
            let entries = self.entries.read().await;
            entries
                .get(&user_id)
                .map(|s| s.snapshot())
                .context("user_id not found")?
        };

        // 获取锁并执行刷新
        {
            let entries = self.entries.read().await;
            if let Some(state) = entries.get(&user_id) {
                let _guard = state.refresh_lock.lock().await;

                // 双重检查
                {
                    let entries = self.entries.read().await;
                    if let Some(state) = entries.get(&user_id) {
                        let threshold =
                            Utc::now() + Duration::seconds(self.refresh_before_expiry_secs);
                        if state.access_token_expire_at > threshold {
                            return Ok((user_id, state.access_token.clone()));
                        }
                    }
                }

                // 尝试刷新token
                if snapshot.refresh_token_expire_at > Utc::now() {
                    let resp = self
                        .http
                        .get(format!("{}/auth/access-token", self.base_url))
                        .header("x-user-id", user_id.to_string())
                        .header("x-refresh-token", &snapshot.refresh_token)
                        .send()
                        .await
                        .context("refresh request failed")?;

                    let api_resp: ApiResponse<AccessTokenResponse> =
                        resp.json().await.context("refresh parse failed")?;
                    if let Some(token_resp) = api_resp.data {
                        let mut entries = self.entries.write().await;
                        if let Some(state) = entries.get_mut(&user_id) {
                            state.access_token = token_resp.access_token;
                            state.access_token_expire_at = token_resp.access_token_expire_at;
                            return Ok((user_id, state.access_token.clone()));
                        }
                    }
                }

                // refresh_token也过期了
                anyhow::bail!(
                    "tokens expired for user_id {} (account: {}), re-login required",
                    user_id,
                    snapshot.account
                );
            }
        }

        anyhow::bail!("user_id {} not found in token store", user_id)
    }

    /// Re-login a user, replacing stale token state (e.g. after password change).
    pub async fn relogin(&self, account: &str, password: &str) -> Result<i64> {
        // Login will overwrite the existing entry for this user_id
        let uid = self.login(account, password).await?;
        Ok(uid)
    }

    pub async fn warmup(&self, credentials: &[(&str, &str)]) -> Result<Vec<i64>> {
        let mut user_ids = Vec::with_capacity(credentials.len());
        for (account, password) in credentials {
            let uid = self.login(account, password).await?;
            user_ids.push(uid);
        }
        Ok(user_ids)
    }

    /// 并发登录多个用户（提高效率）
    pub async fn warmup_concurrent(
        &self,
        credentials: &[(&str, &str)],
        concurrency: usize,
    ) -> Result<Vec<i64>> {
        use futures::stream::{self, StreamExt};

        let results: Vec<Result<i64>> = stream::iter(credentials.iter())
            .map(|(account, password)| async move {
                self.login(account, password).await
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        let mut user_ids = Vec::with_capacity(results.len());
        for result in results {
            user_ids.push(result?);
        }
        Ok(user_ids)
    }

    /// 批量获取多个用户的认证信息
    pub async fn batch_get_auth(&self, user_ids: &[i64]) -> Result<Vec<(i64, String)>> {
        let mut results = Vec::with_capacity(user_ids.len());
        for &user_id in user_ids {
            let auth = self.get_auth(user_id).await?;
            results.push(auth);
        }
        Ok(results)
    }

    /// 并发获取多个用户的认证信息
    pub async fn batch_get_auth_concurrent(
        &self,
        user_ids: &[i64],
        concurrency: usize,
    ) -> Result<Vec<(i64, String)>> {
        use futures::stream::{self, StreamExt};

        let results: Vec<Result<(i64, String)>> = stream::iter(user_ids.iter())
            .map(|&user_id| async move { self.get_auth(user_id).await })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        let mut auth_results = Vec::with_capacity(results.len());
        for result in results {
            auth_results.push(result?);
        }
        Ok(auth_results)
    }

    /// 重新登录用户（用于密码修改等场景）
    pub async fn relogin_with_password(
        &self,
        user_id: i64,
        new_password: &str,
    ) -> Result<()> {
        let account = self.get_account(user_id).await?;
        self.relogin(&account, new_password).await?;
        Ok(())
    }
}
