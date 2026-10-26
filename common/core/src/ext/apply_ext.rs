pub trait Apply<Res> {
    /// 对自身调用一次闭包并返回其结果
    #[inline]
    fn apply<F: FnOnce(Self) -> Res>(self, f: F) -> Res
    where
        Self: Sized,
    {
        f(self)
    }
}

impl<T: ?Sized, Res> Apply<Res> for T {}
