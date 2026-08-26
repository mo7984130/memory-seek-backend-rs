use common::time::Duration;

pub const ACCESS_TOKEN_EXPIRE: Duration = Duration::from_secs(2 * 60 * 60);
pub const REFRESH_TOKEN_EXPIRE: Duration = Duration::from_hours(24 * 30);
pub const EMAIL_CODE_EXPIRE: Duration = Duration::from_mins(10);
