//! 凭据存储抽象与登录态管理。
//!
//! [`Storage`] 由调用方注入(文件、钥匙串、内存等);[`AuthStorage`] 在其之上
//! 提供与前端的 `AuthStorage` 对齐的登录态读写。

mod memory;

#[cfg(feature = "file-storage")]
mod file;

pub use memory::MemoryStorage;

#[cfg(feature = "file-storage")]
pub use file::FileStorage;

use std::sync::Arc;

use common_core::time::{DateTime, now};
use types_core::UserId;
use types_identity::auth::LoginResponse;

/// 存储操作错误。
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// 底层 IO 失败(读写文件、创建目录等)。
    #[error("存储 IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化 / 反序列化失败(如存储文件损坏)。
    #[error("存储序列化错误: {0}")]
    Serde(#[from] serde_json::Error),
}

/// 键值存储抽象:由调用方注入具体实现。
///
/// 实现需自行处理并发(方法只取 `&self`)。
pub trait Storage: Send + Sync {
    /// 读取键对应的值;键不存在返回 `Ok(None)`。
    fn get(&self, key: &str) -> Result<Option<String>, StorageError>;
    /// 写入键值。
    fn set(&self, key: &str, value: String) -> Result<(), StorageError>;
    /// 删除键(键不存在视为成功)。
    fn remove(&self, key: &str) -> Result<(), StorageError>;
}

/// 让共享句柄也能作为 [`Storage`] 注入(如 `Arc<MyStorage>` / `Arc<dyn Storage>`)。
impl<T: Storage + ?Sized> Storage for Arc<T> {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        (**self).get(key)
    }

    fn set(&self, key: &str, value: String) -> Result<(), StorageError> {
        (**self).set(key, value)
    }

    fn remove(&self, key: &str) -> Result<(), StorageError> {
        (**self).remove(key)
    }
}

/// 让装箱实现也能作为 [`Storage`] 注入(如 `Box<dyn Storage>`)。
impl<T: Storage + ?Sized> Storage for Box<T> {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        (**self).get(key)
    }

    fn set(&self, key: &str, value: String) -> Result<(), StorageError> {
        (**self).set(key, value)
    }

    fn remove(&self, key: &str) -> Result<(), StorageError> {
        (**self).remove(key)
    }
}

/// 存储键名(与 TS 客户端保持一致)。
mod keys {
    pub const USER_ID: &str = "MemorySeek.userId";
    pub const ACCESS_TOKEN: &str = "MemorySeek.accessToken";
    pub const ACCESS_TOKEN_EXPIRE_AT: &str = "MemorySeek.accessTokenExpireAt";
    pub const REFRESH_TOKEN: &str = "MemorySeek.refreshToken";
    pub const REFRESH_TOKEN_EXPIRE_AT: &str = "MemorySeek.refreshTokenExpireAt";
}

/// 登录态读写:包装一个 [`Storage`],仅管理「用户 ID + 两类 token」。
///
/// 用户资料不在此持久化(由上层自行保存);token 本质是缓存,读取缺失或过期
/// 时按「未登录」处理,最坏只需重新登录。
#[derive(Clone)]
pub struct AuthStorage {
    storage: Arc<dyn Storage>,
}

impl AuthStorage {
    /// 包装一个存储实现。
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// 保存登录结果:只落 user_id 与 access / refresh token,不持久化用户资料。
    pub fn set_login_result(&self, result: &LoginResponse) -> Result<(), StorageError> {
        self.set_user_id(result.user.id)?;
        self.set_access_token(&result.access_token, result.access_token_expire_at)?;
        self.set_refresh_token(&result.refresh_token, result.refresh_token_expire_at)?;
        Ok(())
    }

    /// 写入当前登录用户 ID。
    pub fn set_user_id(&self, user_id: UserId) -> Result<(), StorageError> {
        self.storage.set(keys::USER_ID, user_id.to_string())
    }

    /// 当前登录用户 ID;缺失或无法解析时返回 `Ok(None)`。
    pub fn user_id(&self) -> Result<Option<UserId>, StorageError> {
        Ok(self
            .storage
            .get(keys::USER_ID)?
            .and_then(|raw| raw.parse().ok()))
    }

    /// 移除用户 ID。
    pub fn remove_user_id(&self) -> Result<(), StorageError> {
        self.storage.remove(keys::USER_ID)
    }

    /// 清除全部登录信息(用户 ID 与两类 token)。
    pub fn clear_login_result(&self) -> Result<(), StorageError> {
        self.remove_user_id()?;
        self.remove_access_token()?;
        self.remove_refresh_token()?;
        Ok(())
    }

    /// 写入 access token 与其过期时间。
    pub fn set_access_token(
        &self,
        access_token: &str,
        expire_at: DateTime,
    ) -> Result<(), StorageError> {
        self.storage
            .set(keys::ACCESS_TOKEN, access_token.to_owned())?;
        self.storage.set(
            keys::ACCESS_TOKEN_EXPIRE_AT,
            expire_at.timestamp().to_string(),
        )?;
        Ok(())
    }

    /// 读取 access token;已过期则清除并返回 `Ok(None)`。
    pub fn access_token(&self) -> Result<Option<String>, StorageError> {
        let Some(expire_at) = self.storage.get(keys::ACCESS_TOKEN_EXPIRE_AT)? else {
            return Ok(None);
        };
        if is_expired(&expire_at) {
            self.remove_access_token()?;
            return Ok(None);
        }
        self.storage.get(keys::ACCESS_TOKEN)
    }

    /// 移除 access token。
    pub fn remove_access_token(&self) -> Result<(), StorageError> {
        self.storage.remove(keys::ACCESS_TOKEN)?;
        self.storage.remove(keys::ACCESS_TOKEN_EXPIRE_AT)?;
        Ok(())
    }

    /// 写入 refresh token 与其过期时间。
    pub fn set_refresh_token(
        &self,
        refresh_token: &str,
        expire_at: DateTime,
    ) -> Result<(), StorageError> {
        self.storage
            .set(keys::REFRESH_TOKEN, refresh_token.to_owned())?;
        self.storage.set(
            keys::REFRESH_TOKEN_EXPIRE_AT,
            expire_at.timestamp().to_string(),
        )?;
        Ok(())
    }

    /// 读取 refresh token;已过期则清除并返回 `Ok(None)`。
    pub fn refresh_token(&self) -> Result<Option<String>, StorageError> {
        let Some(expire_at) = self.storage.get(keys::REFRESH_TOKEN_EXPIRE_AT)? else {
            return Ok(None);
        };
        if is_expired(&expire_at) {
            self.remove_refresh_token()?;
            return Ok(None);
        }
        self.storage.get(keys::REFRESH_TOKEN)
    }

    /// 移除 refresh token。
    pub fn remove_refresh_token(&self) -> Result<(), StorageError> {
        self.storage.remove(keys::REFRESH_TOKEN)?;
        self.storage.remove(keys::REFRESH_TOKEN_EXPIRE_AT)?;
        Ok(())
    }

    /// 是否处于登录态(有用户 ID 且 refresh token 未过期)。
    pub fn check_login(&self) -> Result<bool, StorageError> {
        Ok(self.user_id()?.is_some() && self.refresh_token()?.is_some())
    }
}

/// 判断以 Unix 秒存储的过期时间是否已到期(无法解析时按已过期处理)。
fn is_expired(expire_at_secs: &str) -> bool {
    match expire_at_secs.parse::<i64>() {
        Ok(secs) => secs <= now().timestamp(),
        Err(_) => true,
    }
}
