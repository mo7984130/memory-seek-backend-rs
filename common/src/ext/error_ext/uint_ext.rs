use crate::error::AppError;
use crate::ext::log_warn;

pub trait UintExt: Sized {
    #[track_caller]
    fn zero_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        err: AppError,
    ) -> Result<Self, AppError>;

    #[track_caller]
    fn no_zero_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        err: AppError,
    ) -> Result<Self, AppError>;
}

macro_rules! impl_rows_affected_ext {
    ($($t:ty),* $(,)?) => {
        $(
            impl UintExt for $t {
                #[track_caller]
                fn zero_or_warn(self,
                    reason: &'static str,
                    context: &'static str,
                    err: AppError,
                ) -> Result<Self, AppError> {
                    if self != 0 {
                        Err(log_warn(reason, context, err))
                    } else {
                        Ok(self)
                    }
                }

                #[track_caller]
                fn no_zero_or_warn(self,
                    reason: &'static str,
                    context: &'static str,
                    err: AppError,
                ) -> Result<Self, AppError> {
                    if self == 0 {
                        Err(log_warn(reason, context, err))
                    } else {
                        Ok(self)
                    }
                }
            }
        )*
    };
}

impl_rows_affected_ext!(u8, u16, u32, u64, u128, usize);
