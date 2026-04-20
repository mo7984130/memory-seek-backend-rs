use crate::utils::{Argon2idConfig, HashAlgorithm};

pub const HASHER: HashAlgorithm = HashAlgorithm::Argon2id(
    Argon2idConfig {
        m_cost: 16 * 1024,
        t_cost: 2,
        p_cost: 1
    }
);
