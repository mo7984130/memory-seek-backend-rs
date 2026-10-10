//! 凭据存储抽象与登录态管理。
//!
//! [`Storage`] 由调用方注入(文件、钥匙串、内存等);[`AuthStorage`] 在其之上
//! 提供与前端的 `AuthStorage` 对齐的登录态读写。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use common_core::time::{DateTime, now};
use types_core::UserId;
use types_identity::auth::LoginResponse;
use types_identity::user::UserInfo;

/// 键值存储抽象:由调用方注入具体实现。
///
/// 实现需自行处理并发(方法只取 `&self`)。
pub trait Storage: Send + Sync {
    /// 读取键对应的值。
    fn get(&self, key: &str) -> Option<String>;
    /// 写入键值。
    fn set(&self, key: &str, value: String);
    /// 删除键。
    fn remove(&self, key: &str);
}

/// 基于内存的 [`Storage`] 实现:便于测试与临时场景,进程退出即丢失。
#[derive(Default)]
pub struct MemoryStorage {
    map: Mutex<HashMap<String, String>>,
}

impl MemoryStorage {
    /// 创建空的内存存储。
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for MemoryStorage {
    fn get(&self, key: &str) -> Option<String> {
        self.map.lock().unwrap().get(key).cloned()
    }

    fn set(&self, key: &str, value: String) {
        self.map.lock().unwrap().insert(key.to_owned(), value);
    }

    fn remove(&self, key: &str) {
        self.map.lock().unwrap().remove(key);
    }
}

/// 存储键名(与 TS 客户端保持一致)。
mod keys {
    pub const ACCESS_TOKEN: &str = "MemorySeek.accessToken";
    pub const ACCESS_TOKEN_EXPIRE_AT: &str = "MemorySeek.accessTokenExpireAt";
    pub const REFRESH_TOKEN: &str = "MemorySeek.refreshToken";
    pub const REFRESH_TOKEN_EXPIRE_AT: &str = "MemorySeek.refreshTokenExpireAt";
    pub const LOGIN_RESULT: &str = "MemorySeek.loginResult";
}

/// 登录态读写:包装一个 [`Storage`],提供 token 与登录结果的便捷操作。
#[derive(Clone)]
pub struct AuthStorage {
    storage: Arc<dyn Storage>,
}

impl AuthStorage {
    /// 包装一个存储实现。
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// 保存登录结果,并同步写入 access / refresh token。
    pub fn set_login_result(&self, result: &LoginResponse) {
        if let Ok(json) = serde_json::to_string(result) {
            self.storage.set(keys::LOGIN_RESULT, json);
        }
        self.set_access_token(&result.access_token, result.access_token_expire_at);
        self.set_refresh_token(&result.refresh_token, result.refresh_token_expire_at);
    }

    /// 读取登录结果。
    pub fn get_login_result(&self) -> Option<LoginResponse> {
        self.storage
            .get(keys::LOGIN_RESULT)
            .and_then(|raw| serde_json::from_str(&raw).ok())
    }

    /// 当前登录用户 ID。
    pub fn user_id(&self) -> Option<UserId> {
        self.get_login_result().map(|r| r.user.id)
    }

    /// 当前登录用户信息。
    pub fn user(&self) -> Option<UserInfo> {
        self.get_login_result().map(|r| r.user)
    }

    /// 清除全部登录信息(登录结果与两类 token)。
    pub fn clear_login_result(&self) {
        self.storage.remove(keys::LOGIN_RESULT);
        self.remove_access_token();
        self.remove_refresh_token();
    }

    /// 写入 access token 与其过期时间。
    pub fn set_access_token(&self, access_token: &str, expire_at: DateTime) {
        self.storage
            .set(keys::ACCESS_TOKEN, access_token.to_owned());
        self.storage.set(
            keys::ACCESS_TOKEN_EXPIRE_AT,
            expire_at.timestamp().to_string(),
        );
    }

    /// 读取 access token;已过期则清除并返回 `None`。
    pub fn access_token(&self) -> Option<String> {
        let expire_at = self.storage.get(keys::ACCESS_TOKEN_EXPIRE_AT)?;
        if is_expired(&expire_at) {
            self.remove_access_token();
            return None;
        }
        self.storage.get(keys::ACCESS_TOKEN)
    }

    /// 移除 access token。
    pub fn remove_access_token(&self) {
        self.storage.remove(keys::ACCESS_TOKEN);
        self.storage.remove(keys::ACCESS_TOKEN_EXPIRE_AT);
    }

    /// 写入 refresh token 与其过期时间。
    pub fn set_refresh_token(&self, refresh_token: &str, expire_at: DateTime) {
        self.storage
            .set(keys::REFRESH_TOKEN, refresh_token.to_owned());
        self.storage.set(
            keys::REFRESH_TOKEN_EXPIRE_AT,
            expire_at.timestamp().to_string(),
        );
    }

    /// 读取 refresh token;已过期则清除并返回 `None`。
    pub fn refresh_token(&self) -> Option<String> {
        let expire_at = self.storage.get(keys::REFRESH_TOKEN_EXPIRE_AT)?;
        if is_expired(&expire_at) {
            self.remove_refresh_token();
            return None;
        }
        self.storage.get(keys::REFRESH_TOKEN)
    }

    /// 移除 refresh token。
    pub fn remove_refresh_token(&self) {
        self.storage.remove(keys::REFRESH_TOKEN);
        self.storage.remove(keys::REFRESH_TOKEN_EXPIRE_AT);
    }

    /// 是否处于登录态(有登录结果且 refresh token 未过期)。
    pub fn check_login(&self) -> bool {
        self.get_login_result().is_some() && self.refresh_token().is_some()
    }
}

/// 判断以 Unix 秒存储的过期时间是否已到期(无法解析时按已过期处理)。
fn is_expired(expire_at_secs: &str) -> bool {
    match expire_at_secs.parse::<i64>() {
        Ok(secs) => secs <= now().timestamp(),
        Err(_) => true,
    }
}
