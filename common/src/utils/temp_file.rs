//! 通用临时文件守卫。
//!
//! 用于"流式写盘后再消费"的场景(如文件上传落盘、后续任务读取),
//! 保证任何路径下临时文件都会被清理; 通过 [`TempFile::into_persisted`]
//! 显式移交所有权后不再自动删除。

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use tracing::warn;

/// 临时文件守卫: 创建后持有路径与写句柄, [`Drop`] 时自动删除。
pub struct TempFile {
    /// 文件路径; 移交所有权或 Drop 后为 `None`
    path: Option<PathBuf>,
    /// 打开的写句柄; 消费阶段可从 `path()` 重新打开
    file: Option<File>,
}

impl TempFile {
    /// 在指定目录下以给定文件名创建临时文件(如调用方的 uuid)。
    ///
    /// 目录需已存在, 由调用方保证(如服务启动时创建)。
    pub fn create_in(dir: &Path, name: &str) -> io::Result<Self> {
        let path = dir.join(name);
        let file = File::create(&path)?;
        Ok(Self {
            path: Some(path),
            file: Some(file),
        })
    }

    /// 临时文件路径。
    pub fn path(&self) -> &Path {
        self.path.as_deref().expect("TempFile 已移交或已释放")
    }

    /// 写句柄(需可变)。
    pub fn file(&mut self) -> &mut File {
        self.file.as_mut().expect("TempFile 已移交或已释放")
    }

    /// 为临时文件追加扩展名并更新内部路径(如嗅探/校验出真实格式后重命名)。
    ///
    /// 重命名前会关闭写句柄; 之后 [`Drop`] 删除的是新路径。
    pub fn set_extension(&mut self, ext: &str) -> io::Result<()> {
        self.file.take();
        let Some(path) = self.path.as_ref() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "TempFile 已移交或已释放",
            ));
        };
        let new_path = path.with_extension(ext);
        if new_path != *path {
            std::fs::rename(path, &new_path)?;
            self.path = Some(new_path);
        }
        Ok(())
    }

    /// 移交所有权: 返回路径, 之后 [`Drop`] 不再删除文件。
    pub fn into_persisted(mut self) -> PathBuf {
        // 先关闭写句柄
        self.file.take();
        self.path.take().expect("TempFile 已移交")
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        // 先关闭句柄再删除, 避免 Windows 上删除被占用的文件失败
        self.file.take();
        if let Some(path) = self.path.take()
            && let Err(error) = std::fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            warn!(path = %path.display(), error = %error, "清理临时文件失败");
        }
    }
}

/// 删除目录及其全部内容, 用于优雅关闭时清理临时文件。
///
/// 目录不存在时视为已清理; 删除失败仅记录告警, 不阻断关闭流程。
pub fn remove_dir_all(dir: &Path) {
    if let Err(error) = std::fs::remove_dir_all(dir)
        && error.kind() != io::ErrorKind::NotFound
    {
        warn!(path = %dir.display(), error = %error, "清理临时目录失败");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试临时文件创建后存在, Drop 后自动删除
    #[test]
    fn temp_file_removed_on_drop() {
        let dir = temp_dir();
        let path = {
            let temp_file = TempFile::create_in(&dir, "file_a").unwrap();
            let path = temp_file.path().to_owned();
            assert!(path.exists());
            path
        };
        assert!(!path.exists());
    }

    /// 测试移交所有权后文件不再被删除
    #[test]
    fn temp_file_persisted_after_into_persisted() {
        let dir = temp_dir();
        let path = {
            let temp_file = TempFile::create_in(&dir, "file_b").unwrap();
            let path = temp_file.into_persisted();
            assert!(path.exists());
            path
        };
        assert!(path.exists());
        std::fs::remove_file(&path).unwrap();
    }

    /// 测试追加扩展名后路径更新且 Drop 删除新路径
    #[test]
    fn temp_file_set_extension_renames_and_cleans_up() {
        let dir = temp_dir();
        let (old_path, new_path) = {
            let mut temp_file = TempFile::create_in(&dir, "file_c").unwrap();
            let old_path = temp_file.path().to_owned();
            temp_file.set_extension("jpg").unwrap();
            let new_path = temp_file.path().to_owned();
            assert_eq!(
                new_path,
                PathBuf::from(format!("{}.jpg", old_path.display()))
            );
            assert!(!old_path.exists());
            assert!(new_path.exists());
            (old_path, new_path)
        };
        // Drop 后新路径被清理
        assert!(!new_path.exists());
        assert!(!old_path.exists());
        std::fs::remove_dir(&dir).unwrap();
    }

    /// 测试删除目录及其内容
    #[test]
    fn remove_dir_all_removes_dir_and_contents() {
        let dir = temp_dir();
        let temp_file = TempFile::create_in(&dir, "file_d").unwrap();
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("inner.txt"), b"x").unwrap();

        remove_dir_all(&dir);

        assert!(!temp_file.path().exists());
        assert!(!nested.exists());
        assert!(!dir.exists());
    }

    /// 测试目录不存在时静默返回
    #[test]
    fn remove_dir_all_ignores_missing_dir() {
        let dir = temp_dir();
        std::fs::remove_dir_all(&dir).unwrap();

        remove_dir_all(&dir);

        assert!(!dir.exists());
    }

    /// 创建独立测试目录, 测试结束后清理
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "temp_file_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
