pub trait ToOptionExt<T> {
    /// 将值包装为 `Some`
    fn to_some(self) -> Option<T>;
}

impl<T> ToOptionExt<T> for T {
    /// 将值包装为 `Some`
    #[inline]
    fn to_some(self) -> Option<T> {
        Some(self)
    }
}
