use common_core::r::R;
use serde::Serialize;

pub trait ToROkExt<T: Serialize, E> {
    /// 将结果中的 `Ok` 值包装为统一响应 `R`，`Err` 原样透传
    fn to_r_ok(self) -> Result<R<T>, E>;
}

impl<T: Serialize, E> ToROkExt<T, E> for Result<T, E> {
    /// 将 `Ok` 值包装为统一响应 `R`，`Err` 原样透传
    #[inline]
    fn to_r_ok(self) -> Result<R<T>, E> {
        match self {
            Ok(v) => Ok(R::ok(v)),
            Err(e) => Err(e),
        }
    }
}
