//! 跨上下文共享的枚举词汇:按所属上下文分子模块。
//!
//! 判据:被多个限界上下文引用、且留在任一域契约 crate 都会形成相互依赖,
//! 因此必须位于各域契约 crate 之下。
//! - `visual`:`VisualKind`(视觉令牌契约 `types-token` 与视觉实体 / DTO 共同引用)
//! - `page`:`PageDirection`(分页方向,属跨上下文通用词汇)
//!
//! 子模块保持私有并在此重导出,避免与 [`crate::ids`] 下的同名模块在 crate 根撞名。

mod page;
mod visual;

pub use page::*;
pub use visual::*;
