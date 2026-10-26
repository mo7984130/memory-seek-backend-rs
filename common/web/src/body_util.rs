//! 请求体相关的辅助工具。

use http_body_util::LengthLimitError;

/// 沿 `source()` 链判断错误链中是否包含请求体长度超限错误(`DefaultBodyLimit` 触发)
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
