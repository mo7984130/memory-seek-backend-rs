use common_redis::Pool;
use metrics::gauge;

/// 采集 Redis 连接池指标
///
/// deadpool-redis 提供 status() 方法获取连接池状态
pub fn collect_redis_metrics(pool: &Pool) {
    let status = pool.status();
    // deadpool 的 `size` 为连接总数（含空闲），active 需扣除空闲，与 database 连接池语义对齐。
    let idle = status.available;
    gauge!("redis.connections.active").set(status.size.saturating_sub(idle) as f64);
    gauge!("redis.connections.idle").set(idle as f64);
    gauge!("redis.connections.waiting").set(status.waiting as f64);
}
