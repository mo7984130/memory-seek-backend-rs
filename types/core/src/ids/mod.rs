//! 强类型 ID:按限界上下文分子模块声明。
//!
//! ID 是跨上下文交互的通用语言(外键列、DTO 字段、游标),因此类型本身属于共享内核、
//! 集中在本 crate,以保证线格式(JSON 序列化为字符串)、解析错误(`ParseIdError`)
//! 与 `ts` / `orm` 派生只有一份真相。
//!
//! 子模块的划分只表示"这个 ID 描述哪个上下文的主键",不改变类型的归属:
//! - `identity`:用户身份(`UserId`、`AdminId`)
//! - `audit`:审计(`AuditId`)
//! - `visual`:视觉(`VisualId` 及相册 / 评论 / 人脸等实体的 ID)
//!
//! 各域契约 crate 重新导出这些类型以保持既有路径(如 `types_visual::VisualId`、
//! `types_audit::AuditId`)。新增 ID 时按上下文加进对应子模块。
//!
//! 子模块保持私有并在此重导出,避免与 [`crate::kinds`] 下的同名模块在 crate 根撞名。

use thiserror::Error;

#[cfg(feature = "orm")]
use common_core::error::AppError;

mod audit;
mod identity;
mod visual;

pub use audit::*;
pub use identity::*;
pub use visual::*;

/// ID 解析错误（轻量，不依赖 AppError）
///
/// 由 [`id_type!`](crate::id_type) 生成的 `FromStr::Err`。
/// 调用方可按其边界策略转换为 `AppError`。
/// 后端 orm 模式下直接实现 `From<ParseIdError> for AppError` 以便 `?` 直接使用。
#[derive(Error, Debug)]
#[error("{0}")]
pub struct ParseIdError(pub &'static str);

#[cfg(feature = "orm")]
impl From<ParseIdError> for AppError {
    fn from(e: ParseIdError) -> Self {
        AppError::bad_request(e.0)
    }
}
