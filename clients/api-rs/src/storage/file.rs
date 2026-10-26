//! 基于本地文件的 [`Storage`] 实现。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use common_core::JsonFile;

use super::{Storage, StorageError};

/// 基于磁盘文件的 [`Storage`] 实现。
///
/// 全部键值以单个 JSON 对象存于一个文件;构造时一次性读入内存,写入采用
/// 「临时文件 + 原子替换」。存储位置由调用方以显式路径给出,底层复用
/// [`common_core::JsonFile`]。
pub struct FileStorage {
    inner: JsonFile<HashMap<String, String>>,
}

impl FileStorage {
    /// 以显式文件路径构造并立即读入磁盘内容;文件不存在视为空存储,
    /// 父目录在首次写入时自动创建。
    ///
    /// 凭据文件的存放位置由调用方决定(如自行用 `directories` 解析各平台目录)。
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        Ok(Self {
            inner: JsonFile::new(path)?,
        })
    }

    /// 存储文件路径。
    pub fn path(&self) -> &Path {
        self.inner.path()
    }
}

impl Storage for FileStorage {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self.inner.with(|map| map.get(key).cloned()))
    }

    fn set(&self, key: &str, value: String) -> Result<(), StorageError> {
        self.inner.update(|map| {
            map.insert(key.to_owned(), value);
        })?;
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), StorageError> {
        self.inner.update(|map| {
            map.remove(key);
        })?;
        Ok(())
    }
}

/// 把底层通用存储错误按其同类变体透传为 [`StorageError`]。
impl From<common_core::StoreError> for StorageError {
    fn from(error: common_core::StoreError) -> Self {
        match error {
            common_core::StoreError::Io(error) => StorageError::Io(error),
            common_core::StoreError::Serde(error) => StorageError::Serde(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_path(dir: &TempDir) -> PathBuf {
        dir.path().join("credentials.json")
    }

    #[test]
    fn round_trip_persists_across_instances() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);

        let storage = FileStorage::new(&path).unwrap();
        storage.set("a", "1".to_owned()).unwrap();
        storage.set("b", "2".to_owned()).unwrap();
        assert_eq!(storage.get("a").unwrap().as_deref(), Some("1"));

        // 新实例(模拟重启进程)应能从磁盘读回。
        let reopened = FileStorage::new(&path).unwrap();
        assert_eq!(reopened.get("a").unwrap().as_deref(), Some("1"));
        assert_eq!(reopened.get("b").unwrap().as_deref(), Some("2"));
    }

    #[test]
    fn missing_file_reads_as_empty() {
        let dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_path(&dir)).unwrap();
        assert_eq!(storage.get("nope").unwrap(), None);
    }

    #[test]
    fn remove_deletes_key_and_persists() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        let storage = FileStorage::new(&path).unwrap();
        storage.set("k", "v".to_owned()).unwrap();
        storage.remove("k").unwrap();
        assert_eq!(storage.get("k").unwrap(), None);
        assert_eq!(FileStorage::new(&path).unwrap().get("k").unwrap(), None);
    }

    #[test]
    fn corrupt_file_surfaces_serde_error() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        std::fs::write(&path, b"{ not json").unwrap();
        assert!(matches!(
            FileStorage::new(&path),
            Err(StorageError::Serde(_))
        ));
    }

    #[test]
    fn creates_parent_directories_on_write() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested/deeper/credentials.json");
        let storage = FileStorage::new(&path).unwrap();
        storage.set("k", "v".to_owned()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn auth_storage_round_trip_via_file() {
        use crate::storage::AuthStorage;
        use std::sync::Arc;

        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        let storage: Arc<dyn Storage> = Arc::new(FileStorage::new(&path).unwrap());

        let far_future = crate::DateTime::from_timestamp(4_102_444_800, 0).unwrap();
        let result = crate::LoginResponse {
            user: crate::UserInfo {
                id: crate::UserId(7),
                username: "alice".to_owned(),
                nickname: "Alice".to_owned(),
                email: "alice@example.com".to_owned(),
                avatar_token: None,
                created_at: far_future,
            },
            access_token: "access".to_owned(),
            access_token_expire_at: far_future,
            refresh_token: "refresh".to_owned(),
            refresh_token_expire_at: far_future,
        };
        AuthStorage::new(Arc::clone(&storage))
            .set_login_result(&result)
            .unwrap();

        let reopened = AuthStorage::new(Arc::new(FileStorage::new(&path).unwrap()));
        assert!(reopened.check_login().unwrap());
        assert_eq!(reopened.user_id().unwrap(), Some(crate::UserId(7)));
        assert_eq!(reopened.access_token().unwrap().as_deref(), Some("access"));
    }
}
