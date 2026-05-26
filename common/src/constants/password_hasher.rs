//! 密码哈希器常量定义

use crate::utils::{Argon2idConfig, HashAlgorithm};

/// 全局密码哈希器，使用 Argon2id 算法
///
/// 参数配置：`m_cost = 16384`，`t_cost = 2`，`p_cost = 1`，兼顾安全性与性能。
pub const HASHER: HashAlgorithm = HashAlgorithm::Argon2id(Argon2idConfig {
    m_cost: 16 * 1024,
    t_cost: 2,
    p_cost: 1,
});
