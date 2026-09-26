//! 审计上下文的主键(`audit_event` 表)。
//!
//! 审计事实的引用方(actor / target)是裸 `i64`,只有事件自身的主键需要强类型化,
//! 因此本模块只有 `AuditId`。

crate::id_type!(AuditId, "audit/");
