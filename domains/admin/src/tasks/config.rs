use std::time::Duration;

use serde::Deserialize;

/// 后台任务运行时配置。
#[derive(Debug, Clone, Deserialize)]
pub struct AdminJobConfig {
    /// supervisor 轮询队列与回收失联任务的间隔（秒）。
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    /// 执行器续心跳、轮询取消的间隔（秒）。
    #[serde(default = "default_heartbeat_interval_secs")]
    pub heartbeat_interval_secs: u64,
    /// 超过多久没有心跳视为实例失联（秒）。
    #[serde(default = "default_stale_after_secs")]
    pub stale_after_secs: u64,
    /// 单实例同时执行的任务数上限。
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
}

impl Default for AdminJobConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll_interval_secs(),
            heartbeat_interval_secs: default_heartbeat_interval_secs(),
            stale_after_secs: default_stale_after_secs(),
            max_concurrency: default_max_concurrency(),
        }
    }
}

impl AdminJobConfig {
    /// 轮询间隔。
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs.max(1))
    }

    /// 失联判定阈值；至少为 2 秒。
    pub fn stale_after(&self) -> Duration {
        Duration::from_secs(self.stale_after_secs.max(2))
    }

    /// 心跳间隔，强制不超过失联阈值的一半，避免配置错误导致任务被误回收。
    pub fn heartbeat_interval(&self) -> Duration {
        let stale_after = self.stale_after();
        let configured = Duration::from_secs(self.heartbeat_interval_secs.max(1));
        configured.min(stale_after / 2)
    }

    /// 单实例并发上限，至少为 1。
    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency.max(1)
    }
}

/// 返回默认轮询间隔（秒）
const fn default_poll_interval_secs() -> u64 {
    5
}

/// 返回默认心跳间隔（秒）
const fn default_heartbeat_interval_secs() -> u64 {
    10
}

/// 返回默认失联阈值（秒）
const fn default_stale_after_secs() -> u64 {
    30
}

/// 返回默认单实例并发上限
const fn default_max_concurrency() -> usize {
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_is_shorter_than_half_the_stale_threshold() {
        let config = AdminJobConfig {
            stale_after_secs: 30,
            heartbeat_interval_secs: 25,
            ..AdminJobConfig::default()
        };
        assert_eq!(config.heartbeat_interval(), Duration::from_secs(15));
    }

    #[test]
    fn zero_values_are_clamped() {
        let config = AdminJobConfig {
            poll_interval_secs: 0,
            heartbeat_interval_secs: 0,
            stale_after_secs: 0,
            max_concurrency: 0,
        };
        assert_eq!(config.poll_interval(), Duration::from_secs(1));
        assert_eq!(config.stale_after(), Duration::from_secs(2));
        assert_eq!(config.heartbeat_interval(), Duration::from_secs(1));
        assert_eq!(config.max_concurrency(), 1);
    }
}
