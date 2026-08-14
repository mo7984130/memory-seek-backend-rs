use common::error::{AppError, contextual::Result};
use common::ext::ContextOptionExt;

/// 用户领域中必须存在用户的查询结果扩展。
pub(crate) trait UserOptionExt<T> {
    /// 将空查询结果转换为用户域的用户不存在错误.
    fn user_not_found(self) -> Result<T>;
}

impl<T> UserOptionExt<T> for Option<T> {
    fn user_not_found(self) -> Result<T> {
        self.context_warn_none(
            "user_not_found",
            "用户不存在",
            AppError::bad_request("用户不存在"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_user_not_found_returns_bad_request() {
        let result: Result<()> = None.user_not_found();

        assert!(matches!(
            AppError::from(result.unwrap_err()),
            AppError::BadRequest(message) if message == "用户不存在"
        ));
    }

    #[test]
    fn some_user_not_found_returns_value() {
        assert_eq!(Some(42).user_not_found().unwrap(), 42);
    }
}
