mod db_utils;
mod password_hash;
/// 通用工具模块
///
/// 提供项目中常用的工具类型和函数，包括：
///
/// - `rand_utils`: 随机数工具
/// - `db_utils`: 数据库工具（`DbUtils`）
/// - `validators`: 输入验证器（账号、用户名、邮箱、密码、常规字符）
/// - `file_validator`: 文件验证（`FileValidator`）
/// - `password`: 密码哈希工具（Argon2id、Bcrypt）
/// - `token_cipher`: 通用 token 加解密
/// - `metrics_ext`: 性能监控工具（仅 `metrics` feature）
pub mod rand_utils;
mod token_cipher;
#[cfg(feature = "validators")]
pub mod validators;

#[cfg(feature = "file_validator")]
mod file_validator;
#[cfg(feature = "file_validator")]
pub use file_validator::FileValidator;

pub use db_utils::DbUtils;

pub use password_hash::{Argon2idConfig, BcryptConfig, HashAlgorithm};
pub use token_cipher::{TokenCipher, TokenCipherConfig};

#[cfg(feature = "metrics")]
mod metrics_ext;
#[cfg(feature = "metrics")]
pub use metrics_ext::*;
