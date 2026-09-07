use common::{ContextualResult, error::contextual::ext::OptionExt};

pub trait MissDepError<T> {
    fn miss_dep(self, needer_name: &'static str, dep_name: &'static str) -> ContextualResult<T>;
}
impl<T> MissDepError<T> for Option<T> {
    #[inline]
    fn miss_dep(self, needer_name: &'static str, dep_name: &'static str) -> ContextualResult<T> {
        self.ok_or_error(
            "miss_dep",
            format!("{} 需要 {}!", needer_name, dep_name),
            common::AppError::InternalServerError,
        )
    }
}
