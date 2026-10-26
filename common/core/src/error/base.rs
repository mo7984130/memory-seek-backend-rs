use std::borrow::Cow;
use std::fmt::Debug;
use std::panic::Location;
use tracing::Level;

/// 记录错误日志并原样返回传入的载荷；标注调用位置
#[track_caller]
pub(crate) fn log<E>(
    level: Level,
    reason: &'static str,
    context: Cow<'static, str>,
    err: Option<&(dyn Debug + Send + Sync)>,
    payload: E,
) -> E {
    let loc = Location::caller();
    macro_rules! emit {
        ($lvl:ident) => {
            match err {
                Some(e) => tracing::$lvl!(
                    reason,
                    status = "failed",
                    error = ?e,
                    "{context} ({}:{})",
                    loc.file(),
                    loc.line()
                ),
                None => tracing::$lvl!(
                    reason,
                    status = "failed",
                    "{context} ({}:{})",
                    loc.file(),
                    loc.line()
                ),
            }
        };
    }

    match level {
        Level::ERROR => emit!(error),
        Level::WARN => emit!(warn),
        Level::INFO => emit!(info),
        Level::DEBUG => emit!(debug),
        Level::TRACE => emit!(trace),
    }

    payload
}
