//! 管理端共享契约类型。

pub mod dashboard;
pub mod job;
pub mod user;

#[cfg(feature = "orm")]
pub mod job_entity;

#[cfg(feature = "orm")]
#[linkme::distributed_slice(types_db_registry::SCHEMA_PREFIXES)]
static SCHEMA_PREFIX: types_db_registry::SchemaPrefix =
    types_db_registry::SchemaPrefix("types_admin");
