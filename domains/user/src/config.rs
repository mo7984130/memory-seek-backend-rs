use std::time::Duration;

pub const GENERATE_INVITER_CODE_MAX_RETRY: u8 = 3;
pub const INVITER_CODE_LEN: usize = 6;
pub const INVITER_CODE_TTL: Duration = Duration::from_secs(600);

pub const USER_INFO_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
