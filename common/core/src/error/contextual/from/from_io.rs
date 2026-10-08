use crate::error::{AppError, contextual::ContextualError};

impl From<std::io::Error> for ContextualError {
    /// 将 `std::io::Error` 转换为上下文错误
    fn from(error: std::io::Error) -> Self {
        Self::error(
            "io_error",
            "I/O 操作失败",
            error,
            AppError::InternalServerError,
        )
    }
}
