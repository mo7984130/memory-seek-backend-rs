use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // 字段仅在启用 user/photo domain feature 时被消费
pub struct Config {
    /// 是否启用缓存。`false` 时读写全部穿透数据库，便于压测对比
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_local_capacity")]
    pub local_capacity: u64,
    #[serde(default = "default_local_ttl_secs")]
    pub local_ttl_secs: u64,
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
