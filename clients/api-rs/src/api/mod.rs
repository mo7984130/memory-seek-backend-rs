//! 接口分组,与 `memory-seek-api-ts` 的 `src/api` 对齐。
//!
//! 每个分组为若干自由函数,统一以 `&Client` 作为首参:
//!
//! - [`auth`]：登录 / 注册 / 验证码 / 刷新令牌(公开接口)
//! - [`user`]：用户资料、密码、头像、登出等
//! - [`visual`]：影像、收藏夹、评论、时间线、点赞、人脸、人物
//! - [`audit`]：管理端行为审计
//! - [`backup`]：管理端备份与恢复
//! - [`system`]：系统级端点(Prometheus 指标抓取、健康检查等)

pub mod audit;
pub mod auth;
pub mod backup;
pub mod system;
pub mod user;
pub mod visual;
