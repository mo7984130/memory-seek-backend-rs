//! 跨上下文共享的领域通用语言(共享内核)。
//!
//! 只放"被多个限界上下文引用且稳定"的类型:
//! - [`ids`]：强类型 ID(全部由 [`id_type!`] 生成)
//! - [`cursor`]：键集分页游标契约
//! - [`kinds`]：跨上下文共享的枚举(如 `VisualKind`)
//! - [`error`]：ID / 枚举 / 游标的解析错误
//!
//! 约束：`id_type!` 宏与所有 ID 定义必须同 crate —— 宏体内的
//! `#[cfg(feature = "orm" / "ts")]` 按**展开处所在 crate** 的 feature 解析,
//! 跨 crate 展开会导致实体侧与 DTO 侧的派生不一致。新增 ID 一律加在 [`ids`]。

pub mod cursor;
pub mod error;
pub mod ids;
pub mod kinds;
pub mod macros;

/// 强类型 ID 在本 crate 根上再导出一层, 便于消费方直接 `types_core::VisualId`。
pub use ids::*;

/// 跨上下文共享的枚举同样在根上导出(如 `types_core::VisualKind`)。
pub use kinds::*;
