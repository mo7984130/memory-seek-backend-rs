//! 本地 JSON 文件存储:原子读写 + 进程内缓存。
//!
//! 一个 [`JsonFile<T>`] 对应磁盘上一个 JSON 文件,`T` 为任意可序列化类型。
//! 存储位置由调用方以显式路径给出。构造时一次性读入内存,写入采用「临时文件 +
//! 原子替换」,进程内由 [`Mutex`](std::sync::Mutex) 串行化访问(跨进程文件锁
//! 不在支持范围内)。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// 本地存储错误。
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// 底层 IO 失败(读写文件、创建目录等)。
    #[error("存储 IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化 / 反序列化失败(如存储文件损坏)。
    #[error("存储序列化错误: {0}")]
    Serde(#[from] serde_json::Error),
}

/// 以单个 JSON 文件为后端的泛型存储。
///
/// 构造时一次性读入磁盘内容(文件不存在时用给定默认值/工厂产出初始值),写入
/// 采用「临时文件 + 原子替换」,进程内通过 [`Mutex`] 串行化访问。
pub struct JsonFile<T> {
    path: PathBuf,
    data: Mutex<T>,
}

impl<T> JsonFile<T>
where
    T: Serialize + DeserializeOwned,
{
    /// 以显式路径构造,文件不存在时使用 [`Default`] 产出初始值。
    ///
    /// 存储位置由调用方决定(如自行用 `directories` 解析各平台目录)。
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, StoreError>
    where
        T: Default,
    {
        Self::new_with(path, T::default)
    }

    /// 以显式路径构造,文件不存在时用 `default` 工厂产出初始值。
    pub fn new_with(
        path: impl Into<PathBuf>,
        default: impl FnOnce() -> T,
    ) -> Result<Self, StoreError> {
        let path = path.into();
        let data = Self::load(&path)?.unwrap_or_else(default);
        Ok(Self {
            path,
            data: Mutex::new(data),
        })
    }

    /// 存储文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 借出当前值的只读引用执行 `f`(不克隆)。
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.data.lock().unwrap())
    }

    /// 读取当前值的克隆快照。
    pub fn read(&self) -> T
    where
        T: Clone,
    {
        self.data.lock().unwrap().clone()
    }

    /// 整体替换并落盘。
    pub fn write(&self, value: T) -> Result<(), StoreError> {
        let mut data = self.data.lock().unwrap();
        *data = value;
        Self::flush(&data, &self.path)
    }

    /// 就地修改并落盘,返回 `f` 的返回值。
    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, StoreError> {
        let mut data = self.data.lock().unwrap();
        let output = f(&mut data);
        Self::flush(&data, &self.path)?;
        Ok(output)
    }

    /// 从磁盘读取;文件不存在返回 `Ok(None)`。
    fn load(path: &Path) -> Result<Option<T>, StoreError> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// 原子写回磁盘:先写同目录临时文件,再重命名替换。
    fn flush(data: &T, path: &Path) -> Result<(), StoreError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(data)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use tempfile::TempDir;

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg {
        theme: String,
        size: u32,
    }

    fn temp_path(dir: &TempDir) -> PathBuf {
        dir.path().join("config.json")
    }

    #[test]
    fn missing_file_reads_as_default() {
        let dir = TempDir::new().unwrap();
        let store = JsonFile::<Cfg>::new(temp_path(&dir)).unwrap();
        assert_eq!(store.read(), Cfg::default());
    }

    #[test]
    fn missing_file_uses_custom_default() {
        let dir = TempDir::new().unwrap();
        let store = JsonFile::<Cfg>::new_with(temp_path(&dir), || Cfg {
            theme: "light".to_owned(),
            size: 1,
        })
        .unwrap();
        assert_eq!(store.with(|cfg| cfg.theme.clone()), "light");
    }

    #[test]
    fn update_persists_across_instances() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        JsonFile::<Cfg>::new(&path)
            .unwrap()
            .update(|cfg| {
                cfg.theme = "dark".to_owned();
                cfg.size = 12;
            })
            .unwrap();

        let reopened = JsonFile::<Cfg>::new(&path).unwrap();
        assert_eq!(
            reopened.read(),
            Cfg {
                theme: "dark".to_owned(),
                size: 12,
            }
        );
    }

    #[test]
    fn corrupt_file_surfaces_serde_error() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        std::fs::write(&path, b"{ not json").unwrap();
        assert!(matches!(
            JsonFile::<Cfg>::new(&path),
            Err(StoreError::Serde(_))
        ));
    }
}
