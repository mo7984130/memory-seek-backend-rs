#[derive(Debug, Clone, Default)]
pub enum HasChanged<T> {
    #[default]
    Unchanged,
    Changed(T),
}
pub use HasChanged::Changed;
pub use HasChanged::Unchanged;

impl<T> HasChanged<T> {
    #[inline]
    pub fn is_changed(&self) -> bool {
        match self {
            Unchanged => false,
            Changed(_) => true,
        }
    }

    #[inline]
    pub fn is_unchange(&self) -> bool {
        match self {
            Unchanged => true,
            Changed(_) => false,
        }
    }
}
