pub trait OkExt<T, E> {
    fn to_ok(self) -> Result<T, E>;
}

impl<T, E> OkExt<T, E> for T {
    fn to_ok(self) -> Result<T, E> {
        Ok(self)
    }
}
