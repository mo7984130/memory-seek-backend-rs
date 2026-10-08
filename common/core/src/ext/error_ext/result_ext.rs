pub trait ToOk<T, E> {
    /// 将值包装为 `Ok`
    fn to_ok(self) -> std::result::Result<T, E>;
}

pub trait ToErr<T, E> {
    /// 将值包装为 `Err`
    fn to_err(self) -> std::result::Result<T, E>;
}

impl<T, E> ToOk<T, E> for T {
    /// 将值包装为 `Ok`
    #[inline]
    fn to_ok(self) -> std::result::Result<T, E> {
        Ok(self)
    }
}

impl<T, E> ToErr<T, E> for E {
    /// 将值包装为 `Err`
    #[inline]
    fn to_err(self) -> std::result::Result<T, E> {
        Err(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
