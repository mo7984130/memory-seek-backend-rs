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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_to_r_ok_returns_r_with_data() {
        let result = Ok::<i32, String>(42).to_r_ok();
        let r = result.unwrap();
        assert_eq!(r.code, 200);
        assert_eq!(r.data, Some(42));
        assert!(r.msg.is_none());
    }

    #[test]
    fn err_to_r_ok_returns_err() {
        let result = Err::<i32, String>("error".into()).to_r_ok();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "error");
    }

    #[test]
    fn to_ok_wraps_in_ok() {
        let result: Result<i32, String> = 42i32.to_ok();
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn to_err_wraps_in_err() {
        let result: Result<i32, String> = "error".to_string().to_err();
        assert_eq!(result.unwrap_err(), "error");
    }
}
