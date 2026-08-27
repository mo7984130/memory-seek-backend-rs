use common::error::AppError;
use common::error::contextual::Result;
use common::error::contextual::ext::OptionExt;

/// 认证领域中必须存在用户的查询结果扩展。
pub(crate) trait AuthOptionExt<T> {
    /// 将空查询结果转换为认证域的用户不存在错误.
    fn user_not_found(self) -> Result<T>;
}

impl<T> AuthOptionExt<T> for Option<T> {
    fn user_not_found(self) -> Result<T> {
        self.ok_or_warn(
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
    fn some_user_not_found_returns_value() {
        assert_eq!(Some(42).user_not_found().unwrap(), 42);
    }
}
