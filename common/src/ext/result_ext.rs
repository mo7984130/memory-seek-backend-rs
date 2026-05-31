use serde::Serialize;

use crate::r::R;

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

pub trait ToOk<T, E> {
    fn to_ok(self) -> std::result::Result<T, E>;
}

pub trait ToErr<T, E> {
    fn to_err(self) -> std::result::Result<T, E>;
}

impl<T, E> ToOk<T, E> for T {
    fn to_ok(self) -> std::result::Result<T, E> {
        Ok(self)
    }
}

impl<T, E> ToErr<T, E> for E {
    fn to_err(self) -> std::result::Result<T, E> {
        Err(self)
    }
}
