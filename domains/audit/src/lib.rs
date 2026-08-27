//! 事务强一致审计域。
//!
//! 审计域不创建或管理事务。调用方必须把 [`AuditRecorder::append`] 放在自己的
//! `DatabaseTransaction` 中，审计写入失败时由调用方事务统一回滚。
#[cfg(feature = "controller")]
mod mapper;

mod service;
#[cfg(feature = "controller")]
pub use service::AuditQueryer;
pub use service::AuditRecorder;
pub use types::audit::{AuditEvent, AuditRecord};

#[cfg(feature = "controller")]
pub(crate) mod state;
#[cfg(feature = "controller")]
pub use state::AuditState;

#[cfg(feature = "controller")]
pub mod controller;
#[cfg(feature = "controller")]
pub use controller::AuditController;
