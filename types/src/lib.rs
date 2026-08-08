//! types — 合并 entities + memory-seek-type
//!
//! 提供 ID 新类型、DTO、请求参数、SeaORM 实体定义。
//! 通过 feature 控制依赖轻重：
//! - `lightweight`: 仅 ID 类型 + DTO + 校验器（前端/WASM 用）
//! - `orm`: SeaORM 实体定义
//! - `face-engine`: 人脸识别相关实体
//! - `ts`: TypeScript 类型导出

pub mod auth;
pub mod cursor;
pub mod error;
pub mod macros;
pub mod photo;
pub mod user;
pub mod validators;
