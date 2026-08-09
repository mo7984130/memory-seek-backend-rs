use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_local_capacity")]
    pub local_capacity: u64,
    #[serde(default = "default_local_ttl_secs")]
    pub local_ttl_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            local_capacity: default_local_capacity(),
            local_ttl_secs: default_local_ttl_secs(),
        }
    }
}

/// L1 本地缓存最大条目数
const fn default_local_capacity() -> u64 {
    10_000
}

/// L1 本地缓存 TTL（秒）。短 TTL 保证多实例下最终一致。
const fn default_local_ttl_secs() -> u64 {
    60
}
