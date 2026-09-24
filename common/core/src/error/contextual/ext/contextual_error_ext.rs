use crate::error::{ContextualError, ContextualResult};

pub trait IntoContextualExt<T> {
    /// 将基础错误延迟转换为上下文错误.
    fn into_contextual(self) -> ContextualResult<T>;
}

impl<T, E> IntoContextualExt<T> for std::result::Result<T, E>
where
    ContextualError: From<E>,
{
    #[inline]
    fn into_contextual(self) -> ContextualResult<T> {
        self.map_err(ContextualError::from)
    }
}

/// 消费上下文化结果；若为错误则记录其上下文。
///
/// 适用于补偿或清理操作失败时只需记录的场景。
pub trait ContextualResultExt<T> {
    #[track_caller]
    /// 记录错误上下文并丢弃错误结果.
    fn emit_if_err(self);
}

impl<T> ContextualResultExt<T> for ContextualResult<T> {
    #[inline]
    #[track_caller]
    fn emit_if_err(self) {
        if let Err(error) = self {
            error.emit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ContextualResultExt;

    #[test]
    fn emit_if_err_accepts_successful_result() {
        Ok::<(), crate::error::ContextualError>(()).emit_if_err();
    }
}
