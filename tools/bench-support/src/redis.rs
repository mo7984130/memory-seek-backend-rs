use deadpool_redis::{Config, Pool, Runtime};
use redis::AsyncCommands;

/// 默认 Redis 地址:使用专用 db(15),避免误删本地默认库数据
pub const DEFAULT_URL: &str = "redis://127.0.0.1:6379/15";

/// Redis 地址,可通过环境变量 `BENCH_REDIS_URL` 覆盖
pub fn redis_url() -> String {
    std::env::var("BENCH_REDIS_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

/// 构建 Redis 连接池(懒连接,构建时不建立实际连接)
pub fn make_pool() -> Pool {
    let cfg = Config::from_url(redis_url());
    cfg.create_pool(Some(Runtime::Tokio1)).unwrap()
}

/// 探测 Redis 是否可用
pub async fn available(pool: &Pool) -> bool {
    match pool.get().await {
        Ok(mut conn) => conn.ping::<String>().await.is_ok(),
        Err(_) => false,
    }
}

/// 清空当前连接所在 db(FLUSHDB),返回是否成功
pub async fn flush_db(pool: &Pool) -> bool {
    let mut conn = match pool.get().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    redis::cmd("FLUSHDB")
        .query_async::<String>(&mut conn)
        .await
        .is_ok()
}

/// 按前缀清理 key(SCAN + DEL),不影响其它前缀的数据
pub async fn flush_prefix(pool: &Pool, prefix: &str) {
    let mut conn = match pool.get().await {
        Ok(c) => c,
        Err(_) => return,
    };
    let pattern = format!("{}*", prefix);
    let mut keys: Vec<String> = Vec::new();
    {
        let mut iter = match conn.scan_match::<String, String>(pattern).await {
            Ok(i) => i,
            Err(_) => return,
        };
        while let Some(result) = iter.next_item().await {
            if let Ok(key) = result {
                keys.push(key);
            }
        }
    }
    for key in keys {
        let _: redis::RedisResult<usize> = conn.del::<String, usize>(key).await;
    }
}

/// 基准启动前的 Redis 清理入口:连接专用 db 并 FLUSHDB
///
/// 通过环境变量 `BENCH_NO_FLUSH=1` 可跳过清理。
pub fn flush_before_bench() {
    if std::env::var_os("BENCH_NO_FLUSH").is_some() {
        eprintln!("[info] BENCH_NO_FLUSH 已设置,跳过 Redis 清理");
        return;
    }
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = make_pool();
    if !rt.block_on(available(&pool)) {
        eprintln!("[warn] Redis 不可用({}),跳过清理", redis_url());
        return;
    }
    if rt.block_on(flush_db(&pool)) {
        eprintln!("[info] 已清空 Redis db ({})", redis_url());
    } else {
        eprintln!("[warn] 清空 Redis 失败 ({})", redis_url());
    }
}
