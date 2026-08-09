use crate::ext::log_warn;
use crate::{Result, error::AppError};

pub trait UintExt: Sized {
    #[track_caller]
    fn zero_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        err: AppError,
    ) -> Result<Self>;

    #[track_caller]
    fn no_zero_or_warn(
        self,
        reason: &'static str,
        context: &'static str,
        err: AppError,
    ) -> Result<Self>;
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
                ) -> Result<Self> {
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
                ) -> Result<Self> {
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
