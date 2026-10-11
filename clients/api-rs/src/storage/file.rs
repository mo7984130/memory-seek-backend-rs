//! 基于本地文件的 [`Storage`] 实现,跨 Linux / macOS / Windows。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{Storage, StorageError};

/// 凭据文件名(与目录内其它文件区分)。
const FILE_NAME: &str = "credentials.json";

/// 基于磁盘文件的 [`Storage`] 实现。
///
/// 全部键值以单个 JSON 对象存于一个文件;构造时一次性读入内存,写入采用
/// 「临时文件 + 原子替换」。进程内通过 [`Mutex`] 串行化访问(跨进程文件锁
/// 不在支持范围内)。
pub struct FileStorage {
    path: PathBuf,
    map: Mutex<HashMap<String, String>>,
}

impl FileStorage {
    /// 以显式文件路径构造并立即读入磁盘内容;文件不存在视为空存储,
    /// 父目录在首次写入时自动创建。
    ///
    /// 便于测试或需要自定义存放位置的调用方。
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let path = path.into();
        let map = Self::load(&path)?;
        Ok(Self {
            path,
            map: Mutex::new(map),
        })
    }

    /// 以应用名构造,文件落在各平台的用户数据目录下:
    /// - Linux: `$XDG_DATA_HOME/<app>`(缺省 `~/.local/share/<app>`)
    /// - macOS: `~/Library/Application Support/<app>`
    /// - Windows: `%APPDATA%\<app>\data`
    pub fn for_app(app: &str) -> Result<Self, StorageError> {
        let dirs = directories::ProjectDirs::from("", "", app)
            .ok_or_else(|| StorageError::Path(format!("无法定位应用数据目录: {app}")))?;
        Self::new(dirs.data_dir().join(FILE_NAME))
    }

    /// 存储文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 从磁盘读取全部键值;文件不存在时返回空映射。
    fn load(path: &Path) -> Result<HashMap<String, String>, StorageError> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(error) => Err(error.into()),
        }
    }

    /// 把内存缓存原子写回磁盘:先写同目录临时文件,再重命名替换。
    fn flush(map: &HashMap<String, String>, path: &Path) -> Result<(), StorageError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(map)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

impl Storage for FileStorage {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self.map.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &str, value: String) -> Result<(), StorageError> {
        let mut map = self.map.lock().unwrap();
        map.insert(key.to_owned(), value);
        Self::flush(&map, &self.path)
    }

    fn remove(&self, key: &str) -> Result<(), StorageError> {
        let mut map = self.map.lock().unwrap();
        map.remove(key);
        Self::flush(&map, &self.path)
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
