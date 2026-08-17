mod db_utils;
mod password_hash;
/// 通用工具模块
///
/// 提供项目中常用的工具类型和函数，包括：
///
/// - `rand_utils`: 随机数工具
/// - `db_utils`: 数据库工具（`DbUtils`）
/// - `file_validator`: 文件验证（`FileValidator`）
/// - `password`: 密码哈希工具（Argon2id、Bcrypt）
/// - `token_cipher`: 通用 token 加解密
/// - `metrics_ext`: 性能监控工具（`metrics` feature 未启用时变 no-op）
pub mod rand_utils;
mod token_cipher;

#[cfg(feature = "file_validator")]
mod file_validator;
#[cfg(feature = "file_validator")]
pub use file_validator::FileValidator;

pub use db_utils::DbUtils;

#[cfg(feature = "orm")]
pub mod table_metadata;

pub use password_hash::{Argon2idConfig, BcryptConfig, HashAlgorithm};
pub use token_cipher::{TokenCipher, TokenCipherConfig, init_token_cipher, token_cipher};

#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "metrics")]
pub use metrics::{GaugeGuard, MetricsTimer, MetricsTimerExt};
