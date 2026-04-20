use crate::utils::password::{Argon2idConfig, HashAlgorithm};

pub const ACCESS_TOKEN_EXPIRE_SECONDS: i64 = 2 * 60 * 60;
pub const REFRESH_TOKEN_EXPIRE_DAYS: i64 = 30;

pub const HASHER: HashAlgorithm = HashAlgorithm::Argon2id(
    Argon2idConfig {
        m_cost: 16 * 1024,
        t_cost: 2,
        p_cost: 1
    }
);

// 密码验证并发限制
// 0 表示自动检测 (CPU 核心数的一半)
pub const PASSWORD_VERIFY_MAX_CONCURRENCY: usize = 0;
