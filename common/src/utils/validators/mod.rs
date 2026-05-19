/// 输入验证器模块
///
/// 提供统一的账号、用户名、邮箱、密码和常规字符验证功能。
/// 各验证函数返回 `Result<(), ValidationError>`，可与 `validator` crate 的
/// `Validate` trait 配合使用。
///
/// - `account`: 账号验证（自动识别用户名或邮箱）
/// - `username`: 用户名验证（字母、数字、下划线、短横线，4-20 位）
/// - `email`: 邮箱格式验证
/// - `password`: 密码强度验证（8-64 位，必须包含字母和数字）
/// - `normal_chars`: 常规字符验证（禁止 `< > / \ " ' & @` 等特殊符号）
mod account;
mod username;
mod email;
mod password;
mod normal_chars;

pub use account::validate_account;
pub use email::validate_email;
pub use normal_chars::validate_normal_char;
pub use password::validate_password;
pub use username::validate_username;
