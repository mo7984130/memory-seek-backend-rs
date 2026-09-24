//! 跨上下文共享的枚举词汇。
//!
//! 放在共享内核的判据:`VisualKind` 被视觉访问令牌契约(`types-token`)与视觉
//! 实体 / DTO 同时引用,若留在任一域契约 crate 都会形成相互依赖,因此必须
//! 位于两者之下。

use serde::{Deserialize, Serialize};

#[cfg(feature = "orm")]
use sea_orm::entity::prelude::*;

/// 视觉类型:图片 / 视频
///
/// 顶层定义(不依赖 orm):`visual_token` 与 DTO 直接复用,
/// 带 `orm` feature 时附加 SeaORM ActiveEnum 派生,轻量构建仅序列化。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "orm", derive(EnumIter, DeriveActiveEnum))]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "orm",
    sea_orm(rs_type = "String", db_type = "String(StringLen::N(10))")
)]
pub enum VisualKind {
    #[cfg_attr(feature = "orm", sea_orm(string_value = "image"))]
    Image,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "video"))]
    Video,
}

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
