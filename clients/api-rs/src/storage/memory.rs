//! 基于内存的 [`Storage`] 实现。

use std::collections::HashMap;
use std::sync::Mutex;

use super::{Storage, StorageError};

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
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self.map.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &str, value: String) -> Result<(), StorageError> {
        self.map.lock().unwrap().insert(key.to_owned(), value);
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), StorageError> {
        self.map.lock().unwrap().remove(key);
        Ok(())
    }
}
