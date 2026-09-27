//! 用户身份上下文契约:认证 DTO、用户资料 DTO 与 `auth_user` 实体。
//!
//! 认证(`auth`)与用户资料(`user`)共用同一张 user 表与同一套账号校验规则,
//! 因此属于同一个限界上下文。模块布局与既有对外路径保持一致
//! (`types::auth::user::UserId` / `types::auth::models::*` / `types::user::*` /
//! `types::validators::*`),上层 crate 只需重导出。

pub mod auth;
pub mod user;
pub mod validators;

/// DTO 声明宏(定义在共享内核 `types-core`):本 crate 的 DTO 以 `crate::in_dto!`
/// 等路径展开 —— 展开处的 `ts` feature 与 `serde` / `validator` / `ts-rs` 依赖
/// 由本 crate 提供。
pub use types_core::{in_dto, out_dto, validated_newtype};

/// schema 前缀自我登记: `auth_user` 表由 `init_db` 同步。
#[cfg(feature = "orm")]
#[linkme::distributed_slice(types_db_registry::SCHEMA_PREFIXES)]
static SCHEMA_PREFIX: types_db_registry::SchemaPrefix =
    types_db_registry::SchemaPrefix("types_identity");
