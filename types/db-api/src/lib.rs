//! 实体注册契约。
//!
//! 各实体 crate(定义 SeaORM 实体的域契约 crate)通过本 crate 的分布式切片
//! **自我登记**, 避免 schema 编排出中心化的字符串清单:
//!
//! - [`SCHEMA_PREFIXES`]：本 crate 的实体模块路径前缀(如 `"types_visual"`)。
//!   `init_db` 会按前缀逐个调用 `get_schema_registry(prefix).sync(db)` ——
//!   sea-orm 的注册表按 `module_path!()` 字符串前缀匹配, 且 `sync` 只增不删,
//!   因此多次调用安全。
//! - [`INIT_INDEXES`]：幂等索引初始化回调, 由 `common_macros::register_async` 宏登记。
//!
//! ⚠️ 依赖方向:本 crate 必须位于所有实体 crate **之下**(实体要引用切片),
//! 因此这里不能依赖任何 `types-*` 域契约 crate。

use std::pin::Pin;

use common_core::ContextualResult;
use sea_orm::DatabaseConnection;

/// 索引初始化回调的返回类型(装箱的 future)。
pub type InitIndexFuture<'a> = Pin<Box<dyn Future<Output = ContextualResult<()>> + Send + 'a>>;

/// 索引初始化回调签名。
pub type InitIndexFn = for<'a> fn(&'a DatabaseConnection) -> InitIndexFuture<'a>;

/// 幂等索引初始化回调集合(元素由 `common_macros::register_async` 宏登记)。
#[linkme::distributed_slice]
pub static INIT_INDEXES: [InitIndexFn] = [..];

/// 实体模块路径前缀。
///
/// 用 newtype 而非裸 `&'static str`, 以便将来扩展(如附带 crate 名用于日志)。
pub struct SchemaPrefix(pub &'static str);

/// 各实体 crate 自我登记的 schema 前缀集合。
#[linkme::distributed_slice]
pub static SCHEMA_PREFIXES: [SchemaPrefix] = [..];
