//! 基于本地文件的 [`Storage`] 实现,跨 Linux / macOS / Windows。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{Storage, StorageError};

/// 凭据文件名(与目录内其它文件区分)。
const FILE_NAME: &str = "credentials.json";

/// 基于磁盘文件的 [`Storage`] 实现。
///
/// 全部键值以单个 JSON 对象存于一个文件;写入采用「临时文件 + 原子替换」,
/// Unix 下文件权限设为 `0600`。进程内通过 [`Mutex`] 串行化访问(跨进程文件锁
/// 不在支持范围内)。
pub struct FileStorage {
    path: PathBuf,
    state: Mutex<State>,
}

/// 已加载的内存缓存。
struct State {
    map: HashMap<String, String>,
    loaded: bool,
}

impl FileStorage {
    /// 以显式文件路径构造;父目录不存在时在首次写入时自动创建。
    ///
    /// 便于测试或需要自定义存放位置的调用方。
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            state: Mutex::new(State {
                map: HashMap::new(),
                loaded: false,
            }),
        }
    }

    /// 以应用名构造,文件落在各平台的用户数据目录下:
    /// - Linux: `$XDG_DATA_HOME/<app>`(缺省 `~/.local/share/<app>`)
    /// - macOS: `~/Library/Application Support/<app>`
    /// - Windows: `%APPDATA%\<app>\data`
    pub fn for_app(app: &str) -> Result<Self, StorageError> {
        let dirs = directories::ProjectDirs::from("", "", app)
            .ok_or_else(|| StorageError::Path(format!("无法定位应用数据目录: {app}")))?;
        Ok(Self::new(dirs.data_dir().join(FILE_NAME)))
    }

    /// 存储文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 确保内存缓存已从磁盘加载(仅在首次访问时读取)。
    fn ensure_loaded(state: &mut State, path: &Path) -> Result<(), StorageError> {
        if state.loaded {
            return Ok(());
        }
        match std::fs::read(path) {
            Ok(bytes) => state.map = serde_json::from_slice(&bytes)?,
            // 文件尚不存在:视为空存储,首次写入时再创建。
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                state.map = HashMap::new();
            }
            Err(error) => return Err(error.into()),
        }
        state.loaded = true;
        Ok(())
    }

    /// 把内存缓存原子写回磁盘:先写同目录临时文件,再重命名替换。
    fn flush(state: &State, path: &Path) -> Result<(), StorageError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&state.map)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &bytes)?;
        restrict_permissions(&tmp)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

impl Storage for FileStorage {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        let mut state = self.state.lock().unwrap();
        Self::ensure_loaded(&mut state, &self.path)?;
        Ok(state.map.get(key).cloned())
    }

    fn set(&self, key: &str, value: String) -> Result<(), StorageError> {
        let mut state = self.state.lock().unwrap();
        Self::ensure_loaded(&mut state, &self.path)?;
        state.map.insert(key.to_owned(), value);
        Self::flush(&state, &self.path)
    }

    fn remove(&self, key: &str) -> Result<(), StorageError> {
        let mut state = self.state.lock().unwrap();
        Self::ensure_loaded(&mut state, &self.path)?;
        state.map.remove(key);
        Self::flush(&state, &self.path)
    }
}

/// Unix 下把文件权限收紧为 `0600`(仅属主可读写);其它平台依赖系统 ACL。
#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<(), StorageError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<(), StorageError> {
    Ok(())
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

        let storage = FileStorage::new(&path);
        storage.set("a", "1".to_owned()).unwrap();
        storage.set("b", "2".to_owned()).unwrap();
        assert_eq!(storage.get("a").unwrap().as_deref(), Some("1"));

        // 新实例(模拟重启进程)应能从磁盘读回。
        let reopened = FileStorage::new(&path);
        assert_eq!(reopened.get("a").unwrap().as_deref(), Some("1"));
        assert_eq!(reopened.get("b").unwrap().as_deref(), Some("2"));
    }

    #[test]
    fn missing_file_reads_as_empty() {
        let dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_path(&dir));
        assert_eq!(storage.get("nope").unwrap(), None);
    }

    #[test]
    fn remove_deletes_key_and_persists() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        let storage = FileStorage::new(&path);
        storage.set("k", "v".to_owned()).unwrap();
        storage.remove("k").unwrap();
        assert_eq!(storage.get("k").unwrap(), None);
        assert_eq!(FileStorage::new(&path).get("k").unwrap(), None);
    }

    #[test]
    fn corrupt_file_surfaces_serde_error() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        std::fs::write(&path, b"{ not json").unwrap();
        let storage = FileStorage::new(&path);
        assert!(matches!(storage.get("k"), Err(StorageError::Serde(_))));
    }

    #[test]
    fn creates_parent_directories_on_write() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested/deeper/credentials.json");
        let storage = FileStorage::new(&path);
        storage.set("k", "v".to_owned()).unwrap();
        assert!(path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn file_permissions_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        FileStorage::new(&path).set("k", "v".to_owned()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn auth_storage_round_trip_via_file() {
        use crate::storage::AuthStorage;
        use std::sync::Arc;

        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        let storage: Arc<dyn Storage> = Arc::new(FileStorage::new(&path));

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

        let reopened = AuthStorage::new(Arc::new(FileStorage::new(&path)));
        assert!(reopened.check_login().unwrap());
        assert_eq!(reopened.user_id().unwrap(), Some(crate::UserId(7)));
        assert_eq!(reopened.access_token().unwrap().as_deref(), Some("access"));
    }
}
