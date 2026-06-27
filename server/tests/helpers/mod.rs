pub mod app;
pub mod auth;
pub mod db;
pub mod photo;
pub mod photo_like;

use server::config::AppConfig;

/// 加载测试配置
///
/// 从 tests/test.config.json 读取，确保所有测试使用同一份配置。
#[allow(dead_code)]
pub fn test_config() -> AppConfig {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let config_path = format!("{}/tests/test.config.json", manifest_dir);
    std::env::set_var("MEMORY_SEEK_CONFIG_PATH", &config_path);
    AppConfig::load()
}
