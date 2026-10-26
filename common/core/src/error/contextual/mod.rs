mod contextual_error;
pub mod ext;
pub mod from;

use crate::error::AppError;
pub use contextual_error::ContextualError;
pub type ContextualResult<T, E = AppError> = std::result::Result<T, ContextualError<E>>;
pub type Result<T, E = AppError> = std::result::Result<T, ContextualError<E>>;
