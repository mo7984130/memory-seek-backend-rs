//! 分页方向的通用词汇。

use serde::Deserialize;

/// 键集分页方向。
///
/// 属跨上下文通用词汇(视觉 DTO 与 Redis key 构造共同引用),因此下沉到共享内核。
/// TS 导出目录沿用 `visual/`,以保持前端契约不变。
#[derive(Debug, Clone, PartialEq, Eq, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "visual/"))]
pub enum PageDirection {
    Next,
    Prev,
}
