#![allow(dead_code)]

pub mod app;
pub mod auth;
pub mod db;
pub mod photo;
pub mod photo_like;

use server::config::AppConfig;

/// 加载测试配置
///
/// 从 tests/test.config.yaml 读取，确保所有测试使用同一份配置。
pub fn test_config() -> AppConfig {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let config_path = format!("{}/tests/test.config.yaml", manifest_dir);
    AppConfig::load(Some(config_path)).expect("加载测试配置失败")
}
