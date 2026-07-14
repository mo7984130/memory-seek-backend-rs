use deadpool_redis::Pool;
use metrics::gauge;

/// 采集 Redis 连接池指标
///
/// deadpool-redis 提供 status() 方法获取连接池状态
pub fn collect_redis_metrics(pool: &Pool) {
    let status = pool.status();
    gauge!("redis.connections.active").set(status.size as f64);
    gauge!("redis.connections.idle").set(status.available as f64);
    gauge!("redis.connections.waiting").set(status.waiting as f64);
}
