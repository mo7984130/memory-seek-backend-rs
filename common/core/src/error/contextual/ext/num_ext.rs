use std::borrow::Cow;

use crate::error::{ContextualError, ContextualResult};

pub trait UintExt: Sized {
    /// 期望为 0，非 0 则 warn
    fn zero_or_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<Self, E>;

    /// 期望为 0，非 0 则 error
    fn zero_or_error<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<Self, E>;

    /// 期望非 0，为 0 则 warn
    fn nonzero_or_warn<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<Self, E>;

    /// 期望非 0，为 0 则 error
    fn nonzero_or_error<E>(
        self,
        reason: &'static str,
        context: impl Into<Cow<'static, str>>,
        payload: E,
    ) -> ContextualResult<Self, E>;
}

macro_rules! impl_rows_affected_ext {
    ($($t:ty),* $(,)?) => {
        $(
            impl UintExt for $t {
                #[inline]
                fn zero_or_warn<E>(self,
                    reason: &'static str,
                    context: impl Into<Cow<'static, str>>,
                    payload: E,
                ) -> ContextualResult<Self, E> {
                    if self != 0 {
                        Err(ContextualError::warn_without_source(
                            reason,
                            context,
                            payload,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn zero_or_error<E>(self,
                    reason: &'static str,
                    context: impl Into<Cow<'static, str>>,
                    payload: E,
                ) -> ContextualResult<Self, E> {
                    if self != 0 {
                        Err(ContextualError::error_without_source(
                            reason,
                            context,
                            payload,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn nonzero_or_warn<E>(self,
                    reason: &'static str,
                    context: impl Into<Cow<'static, str>>,
                    payload: E,
                ) -> ContextualResult<Self, E> {
                    if self == 0 {
                        Err(ContextualError::warn_without_source(
                            reason,
                            context,
                            payload,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn nonzero_or_error<E>(self,
                    reason: &'static str,
                    context: impl Into<Cow<'static, str>>,
                    payload: E,
                ) -> ContextualResult<Self, E> {
                    if self == 0 {
                        Err(ContextualError::error_without_source(
                            reason,
                            context,
                            payload,
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
