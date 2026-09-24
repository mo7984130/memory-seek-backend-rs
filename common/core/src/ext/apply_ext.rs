pub trait Apply<Res> {
    #[inline]
    fn apply<F: FnOnce(Self) -> Res>(self, f: F) -> Res
    where
        Self: Sized,
    {
        f(self)
    }
}

impl<T: ?Sized, Res> Apply<Res> for T {}
