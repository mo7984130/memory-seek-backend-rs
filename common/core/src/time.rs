pub use std::time::Duration;

pub type DateTime = chrono::DateTime<chrono::Utc>;

/// 返回当前 UTC 时间
pub fn now() -> DateTime {
    chrono::Utc::now()
}

/// 返回 `duration` 之后的 UTC 时间；时长超出 chrono 表示范围时 panic
pub fn after(duration: Duration) -> DateTime {
    now() + chrono::Duration::from_std(duration).expect("duration must fit chrono range")
}
