use common_core::{ContextualResult, error::contextual::ext::OptionExt};

pub trait MissDepError<T> {
    /// 取出依赖值，缺失时返回内部错误
    fn miss_dep(self, needer_name: &'static str, dep_name: &'static str) -> ContextualResult<T>;
}
impl<T> MissDepError<T> for Option<T> {
    /// 取出依赖值，缺失时返回内部错误
    #[inline]
    fn miss_dep(self, needer_name: &'static str, dep_name: &'static str) -> ContextualResult<T> {
        self.ok_or_error(
            "miss_dep",
            format!("{} 需要 {}!", needer_name, dep_name),
            common_core::AppError::InternalServerError,
        )
    }
}
