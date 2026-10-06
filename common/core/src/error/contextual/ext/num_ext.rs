use std::borrow::Cow;

use crate::error::{AppError, ContextualError, ContextualResult};

pub trait UintExt: Sized {
    /// 期望为 0，非 0 则 warn
    fn zero_or_warn(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<Self>;

    /// 期望为 0，非 0 则 error
    fn zero_or_error(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<Self>;

    /// 期望非 0，为 0 则 warn
    fn nonzero_or_warn(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<Self>;

    /// 期望非 0，为 0 则 error
    fn nonzero_or_error(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        app_error: AppError,
    ) -> ContextualResult<Self>;
}

macro_rules! impl_rows_affected_ext {
    ($($t:ty),* $(,)?) => {
        $(
            impl UintExt for $t {
                #[inline]
                fn zero_or_warn(self,
                    reason: &'static str,
                    context: impl Into<Cow<'static, str>>,
                    app_error: AppError,
                ) -> ContextualResult<Self> {
                    if self != 0 {
                        Err(ContextualError::warn_without_source(
                            reason,
                            context,
                            app_error,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn zero_or_error(self,
                    reason: &'static str,
                    context: impl Into<Cow<'static, str>>,
                    app_error: AppError,
                ) -> ContextualResult<Self> {
                    if self != 0 {
                        Err(ContextualError::error_without_source(
                            reason,
                            context,
                            app_error,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn nonzero_or_warn(self,
                    reason: &'static str,
                    context: impl Into<Cow<'static, str>>,
                    app_error: AppError,
                ) -> ContextualResult<Self> {
                    if self == 0 {
                        Err(ContextualError::warn_without_source(
                            reason,
                            context,
                            app_error,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn nonzero_or_error(self,
                    reason: &'static str,
                    context: impl Into<Cow<'static, str>>,
                    app_error: AppError,
                ) -> ContextualResult<Self> {
                    if self == 0 {
                        Err(ContextualError::error_without_source(
                            reason,
                            context,
                            app_error,
                        ))
                    } else {
                        Ok(self)
                    }
                }
            }
        )*
    };
}

impl_rows_affected_ext!(u8, u16, u32, u64, u128, usize);
