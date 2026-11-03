//! 事务强一致审计域。
//!
//! 审计域不创建或管理事务。调用方必须把 [`AuditRecorder::append`] 放在自己的
//! `DatabaseTransaction` 中，审计写入失败时由调用方事务统一回滚。
mod mapper;

mod service;
pub use service::AuditQueryer;
pub use service::AuditRecorder;
pub use types_audit::{AuditEvent, AuditRecord};
