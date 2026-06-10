/// 模块初始化模块
///
/// 负责各业务模块的状态创建和路由挂载：
/// - `database`: 数据库连接初始化
/// - `redis`: Redis 连接池初始化
/// - `log`: 日志系统初始化
/// - `auth`: 认证模块初始化（需要 `auth` feature）
/// - `user`: 用户模块初始化（需要 `user` feature）
/// - `photo`: 照片模块初始化（需要 `photo` feature）
pub mod database;
pub mod redis;

pub mod log;

#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "user")]
pub mod user;

#[cfg(feature = "photo")]
pub mod photo;
