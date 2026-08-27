mod contextual_error;
pub mod ext;
pub mod from;

pub use contextual_error::ContextualError;
pub type ContextualResult<T> = std::result::Result<T, ContextualError>;
pub type Result<T> = std::result::Result<T, ContextualError>;
