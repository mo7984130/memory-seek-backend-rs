use std::time::Duration;

use multi_level_cache::CacheConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_local_capacity")]
    pub local_capacity: u64,
    #[serde(default = "default_local_ttl_secs")]
    pub local_ttl_secs: u64,
}
impl Config {
    pub fn to(&self) -> CacheConfig {
        CacheConfig {
            enabled: self.enabled,
            local_capacity: self.local_capacity,
            local_ttl: Duration::from_secs(self.local_ttl_secs),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            local_capacity: default_local_capacity(),
            local_ttl_secs: default_local_ttl_secs(),
        }
    }
}

/// 默认启用缓存
const fn default_enabled() -> bool {
    true
}

/// L1 本地缓存最大条目数
const fn default_local_capacity() -> u64 {
    10_000
}

/// L1 本地缓存 TTL（秒）。短 TTL 保证多实例下最终一致。
const fn default_local_ttl_secs() -> u64 {
    60
}
