/// 写入带源码调用位置的 warning 日志。
#[macro_export]
macro_rules! caller_warn {
    ($($arg:tt)*) => {
        tracing::warn!(
            caller.file = file!(),
            caller.line = line!(),
            $($arg)*
        )
    };
}

/// 写入带源码调用位置的 error 日志。
#[macro_export]
macro_rules! caller_error {
    ($($arg:tt)*) => {
        tracing::error!(
            caller.file = file!(),
            caller.line = line!(),
            $($arg)*
        )
    };
}
