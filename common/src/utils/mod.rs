/// 通用工具模块
///
/// 提供项目中常用的工具类型和函数，包括：
///
/// - `redis_ext`: Redis 缓存扩展（`CacheExtension`、`RedisExt`）
/// - `result_ext`: Result 类型扩展（`ResultExt`、`ToOkExt`）
/// - `rand_utils`: 随机数工具
/// - `db_utils`: 数据库工具（`DbUtils`）
/// - `validators`: 输入验证器（账号、用户名、邮箱、密码、常规字符）
/// - `file_validator`: 文件验证（`FileValidator`）
/// - `option_ext`: Option 类型扩展（`OptionExt`）
/// - `bool_ext`: Bool 类型扩展（`BoolExt`）
/// - `password`: 密码哈希工具（Argon2id、Bcrypt）
/// - `avatar`: 头像 token 加密
/// - `token_cipher`: 通用 token 加解密
/// - `metrics_ext`: 性能监控工具（仅 `metrics` feature）
mod redis_ext;
mod result_ext;
pub mod rand_utils;
mod db_utils;
pub mod validators;
mod file_validator;
mod option_ext;
mod bool_ext;
mod password;
mod avatar;
mod token_cipher;

pub use bool_ext::BoolExt;
pub use db_utils::DbUtils;
pub use file_validator::FileValidator;
pub use option_ext::OptionExt;
pub use redis_ext::CacheExtension;
pub use redis_ext::RedisExt;
pub use result_ext::ResultExt;
pub use result_ext::ToOkExt;
pub use password::{HashAlgorithm, Argon2idConfig, BcryptConfig};
pub use avatar::encrypt_avatar_token;
pub use token_cipher::{TokenCipher, TokenCipherConfig};

#[cfg(feature = "metrics")]
mod metrics_ext;
#[cfg(feature = "metrics")]
pub use metrics_ext::*;
