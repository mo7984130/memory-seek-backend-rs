use thiserror::Error;

/// ID 解析错误（轻量，不依赖 AppError）
///
/// 调用方可通过 `trace_warn_bad_request` 等方式转换为 `AppError`。
#[derive(Error, Debug)]
#[error("{0}")]
pub struct ParseIdError(pub &'static str);
