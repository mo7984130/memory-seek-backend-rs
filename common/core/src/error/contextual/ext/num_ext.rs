use crate::error::{AppError, ContextualError, ContextualResult};

pub trait UintExt: Sized {
    // 期望为0
    // 非0的话warn
    fn zero_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        err: AppError,
    ) -> ContextualResult<Self>;

    // 期望为0
    // 非0的话error
    fn zero_or_error(
        self,
        reason: &'static str,
        context: &'static str,
        err: AppError,
    ) -> ContextualResult<Self>;

    // 期望为非0
    // 0的话warn
    fn no_zero_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        err: AppError,
    ) -> ContextualResult<Self>;

    // 期望为非0
    // 0的话error
    fn no_zero_or_error(
        self,
        reason: &'static str,
        context: &'static str,
        err: AppError,
    ) -> ContextualResult<Self>;
}

macro_rules! impl_rows_affected_ext {
    ($($t:ty),* $(,)?) => {
        $(
            impl UintExt for $t {
                #[inline]
                fn zero_or_warn(self,
                    reason: &'static str,
                    context: &'static str,
                    err: AppError,
                ) -> ContextualResult<Self> {
                    if self != 0 {
                        Err(ContextualError::warn_without_source(
                            reason,
                            context,
                            err,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn zero_or_error(self,
                    reason: &'static str,
                    context: &'static str,
                    err: AppError,
                ) -> ContextualResult<Self> {
                    if self != 0 {
                        Err(ContextualError::error_without_source(
                            reason,
                            context,
                            err,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn no_zero_or_warn(self,
                    reason: &'static str,
                    context: &'static str,
                    err: AppError,
                ) -> ContextualResult<Self> {
                    if self == 0 {
                        Err(ContextualError::warn_without_source(
                            reason,
                            context,
                            err,
                        ))
                    } else {
                        Ok(self)
                    }
                }

                #[inline]
                fn no_zero_or_error(self,
                    reason: &'static str,
                    context: &'static str,
                    err: AppError,
                ) -> ContextualResult<Self> {
                    if self == 0 {
                        Err(ContextualError::error_without_source(
                            reason,
                            context,
                            err,
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
