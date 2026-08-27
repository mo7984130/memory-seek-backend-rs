use crate::axum::R;
use serde::Serialize;

pub trait ToROkExt<T: Serialize, E> {
    fn to_r_ok(self) -> Result<R<T>, E>;
}

impl<T: Serialize, E> ToROkExt<T, E> for Result<T, E> {
    #[inline]
    fn to_r_ok(self) -> Result<R<T>, E> {
        match self {
            Ok(v) => Ok(R::ok(v)),
            Err(e) => Err(e),
        }
    }
}
