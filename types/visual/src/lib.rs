//! 视觉上下文契约:9 个 SeaORM 实体、视图 / 查询类型与请求 DTO。
//!
//! 依赖方向:`types-core`(强类型 ID、游标、`VisualKind`)+ `types-token`
//! (视觉访问令牌)+ `types-db-registry`(实体注册契约),**不依赖其它上下文契约 crate**。

#![allow(clippy::module_inception)]

pub mod collection;
pub mod collection_visual;
pub mod comment;
pub mod comment_like;
pub mod dto;
pub mod face;
pub mod models;
pub mod person;
pub mod timeline_stat;
pub mod visual;
pub mod visual_like;
pub mod visual_token;

pub use dto::*;
pub use models::*;
pub use visual_token::*;

/// DTO 声明宏(定义在共享内核 `types-core`):本 crate 的 DTO 以 `crate::in_dto!`
/// 等路径展开 —— 展开处的 `ts` feature 与 `serde` / `validator` / `ts-rs` /
/// `derive_more` 依赖由本 crate 提供。
pub use types_core::{in_dto, out_dto, validated_newtype};

/// schema 前缀自我登记: 本 crate 的全部实体表由 `init_db` 同步。
#[cfg(feature = "orm")]
#[linkme::distributed_slice(types_db_registry::SCHEMA_PREFIXES)]
static SCHEMA_PREFIX: types_db_registry::SchemaPrefix =
    types_db_registry::SchemaPrefix("types_visual");
