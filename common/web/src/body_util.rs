//! 请求体相关的辅助工具。

use http_body_util::LengthLimitError;

/// 判断错误链中是否包含请求体长度超限错误(`DefaultBodyLimit` 触发)。
///
/// axum 的 body 流在超过 `DefaultBodyLimit` 时返回包装了
/// `http_body_util::LengthLimitError` 的错误, 可能嵌套多层(如 `axum::Error`
/// → `io::Error` 等), 因此沿 `source()` 链逐层查找。
pub fn is_body_limit_error(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut source = Some(error);
    while let Some(err) = source {
        if err.downcast_ref::<LengthLimitError>().is_some() {
            return true;
        }
        source = err.source();
    }
    false
}
