//! 视觉上下文的共享枚举。

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
