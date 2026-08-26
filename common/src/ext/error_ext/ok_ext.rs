pub trait OkExt<T, E> {
    fn to_ok(self) -> Result<T, E>;
}

impl<T, E> OkExt<T, E> for T {
    #[inline]
    fn to_ok(self) -> Result<T, E> {
        Ok(self)
    }
}

pub trait CollectOkExt: Iterator + Sized {
    #[inline]
    fn collect_ok<T, E>(self) -> Result<T, E>
    where
        T: FromIterator<Self::Item>,
    {
        Ok(self.collect())
    }
}

impl<I> CollectOkExt for I where I: Iterator {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ok_returns_ok_with_value() {
        let result: Result<i32, String> = 42i32.to_ok();
        assert_eq!(result.unwrap(), 42);
    }
}
