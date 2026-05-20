use serde::Serialize;

use crate::r::R;

pub trait ResultExt<T, E> {
    fn to_ok(self) -> Result<T, E>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn to_ok(self) -> Result<T, E> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(e),
        }
    }
}

pub trait ResultRExt<T: Serialize, E> {
    fn to_r_ok(self) -> Result<R<T>, E>;
}

impl<T: Serialize, E> ResultRExt<T, E> for Result<T, E> {
    fn to_r_ok(self) -> Result<R<T>, E> {
        match self {
            Ok(v) => Ok(R::ok(v)),
            Err(e) => Err(e),
        }
    }
}
