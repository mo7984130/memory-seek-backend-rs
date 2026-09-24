//! 视觉访问令牌契约已迁移到跨上下文协议 crate `types-token`,此处重导出以保持
//! 既有路径不变(`types::visual::VisualToken`、`types::visual::visual_token::FaceBBox` 等)。

pub use types_token::*;
