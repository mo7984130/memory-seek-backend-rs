use std::sync::atomic::AtomicU64;

/// 基于时间戳的递增计数器起点
///
/// 用于需要"每次运行使用全新 key 集"的基准场景(如缓存 miss 路径),
/// 避免命中上一次运行回填到 Redis/缓存的旧数据。
pub fn new_counter() -> AtomicU64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    AtomicU64::new(now)
}
