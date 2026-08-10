//! `MultiLevelCache::get_or_load_batch` 基准测试
//!
//! 覆盖路径:
//! - `all_l1_hits`: L1 全命中(热路径,不依赖 Redis)
//! - `disabled`: 缓存整体禁用,直接穿透 loader(不依赖 Redis)
//! - `l1_miss_l2_hit`: 容量 0 强制 L1 miss,L2 命中(需要本地 Redis)
//! - `full_miss`: 全 miss,穿透 loader 并回填 L1/L2(需要本地 Redis)
//!
//! 另有 `*_with_db_io` 变体:loader 内置固定 DB IO 延迟(见 [`DB_IO_DELAY`]),
//! 用于展示真实数据库查询对端到端耗时的影响。
//!
//! Redis 不可用时,依赖 Redis 的场景自动跳过,其余照常运行。
//! 运行: `cargo bench -p common --bench get_or_load_batch`
//! 完整报告(criterion HTML)位于 `target/criterion/report/index.html`。
//!
//! 基础设施(Redis 连接/清理、mock loader、计数器)复用 `bench_support`,
//! 启动时通过 [`bench_support::redis::flush_before_bench`] 清空专用 db。

use std::sync::atomic::Ordering;
use std::time::Duration;

use bench_support::{criterion, delayed_loader, loader, new_counter, redis};
use common::cache::{CacheConfig, MultiLevelCache};
use serde::{Deserialize, Serialize};

const KEY_COUNT: u64 = 100;
const L2_TTL_SECS: u64 = 3600;
/// 模拟一次数据库查询的耗时（与真实 DB 往返量级对齐）
const DB_IO_DELAY: Duration = Duration::from_millis(5);

#[derive(Clone, Serialize, Deserialize)]
struct Row {
    id: u64,
    payload: u64,
}

fn make_cache(
    name: &'static str,
    pool: deadpool_redis::Pool,
    enabled: bool,
    local_capacity: u64,
) -> MultiLevelCache<Row> {
    MultiLevelCache::new(
        name,
        pool,
        CacheConfig::new(enabled, local_capacity, Duration::from_secs(3600)),
    )
}

fn static_keys() -> Vec<u64> {
    (0..KEY_COUNT).collect()
}

fn key_provider(id: &u64) -> String {
    format!("bench:{}", id)
}

fn result_mapper(row: &Row) -> u64 {
    row.id
}

fn make_row(id: u64) -> Row {
    Row { id, payload: 0 }
}

async fn prefill(cache: &MultiLevelCache<Row>, keys: &[u64]) {
    for id in keys {
        cache
            .put(&key_provider(id), make_row(*id), L2_TTL_SECS)
            .await
            .unwrap();
    }
}

fn bench_all_l1_hits(c: &mut criterion::Criterion) {
    let cache = make_cache("bench_all_l1_hits", redis::make_pool(), true, 1024 * 1024);
    let keys = static_keys();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(prefill(&cache, &keys));

    c.bench_function("get_or_load_batch/all_l1_hits", |b| {
        b.to_async(&rt).iter(|| async {
            let result = cache
                .get_or_load_batch(
                    &keys,
                    key_provider,
                    L2_TTL_SECS,
                    |ids| loader(ids, make_row),
                    result_mapper,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn bench_disabled(c: &mut criterion::Criterion) {
    let cache = make_cache("bench_disabled", redis::make_pool(), false, 0);
    let keys = static_keys();
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("get_or_load_batch/disabled", |b| {
        b.to_async(&rt).iter(|| async {
            let result = cache
                .get_or_load_batch(
                    &keys,
                    key_provider,
                    L2_TTL_SECS,
                    |ids| loader(ids, make_row),
                    result_mapper,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn bench_disabled_with_db_io(c: &mut criterion::Criterion) {
    let cache = make_cache("bench_disabled_with_db_io", redis::make_pool(), false, 0);
    let keys = static_keys();
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("get_or_load_batch/disabled_with_db_io", |b| {
        b.to_async(&rt).iter(|| async {
            let result = cache
                .get_or_load_batch(
                    &keys,
                    key_provider,
                    L2_TTL_SECS,
                    |ids| delayed_loader(DB_IO_DELAY, ids, make_row),
                    result_mapper,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn bench_l1_miss_l2_hit(c: &mut criterion::Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = redis::make_pool();
    if !rt.block_on(redis::available(&pool)) {
        eprintln!("[skip] Redis 不可用,跳过 l1_miss_l2_hit");
        return;
    }

    // 容量 0 强制 L1 永不命中,只预填 L2
    let cache = make_cache("bench_l1_miss_l2_hit", pool, true, 0);
    let keys = static_keys();
    rt.block_on(prefill(&cache, &keys));

    c.bench_function("get_or_load_batch/l1_miss_l2_hit", |b| {
        b.to_async(&rt).iter(|| async {
            let result = cache
                .get_or_load_batch(
                    &keys,
                    key_provider,
                    L2_TTL_SECS,
                    |ids| loader(ids, make_row),
                    result_mapper,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn bench_full_miss(c: &mut criterion::Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = redis::make_pool();
    if !rt.block_on(redis::available(&pool)) {
        eprintln!("[skip] Redis 不可用,跳过 full_miss");
        return;
    }

    let cache = make_cache("bench_full_miss", pool, true, 0);
    // 每次迭代使用全新 key 集,保证 L2 必 miss(避免回填后二次命中)
    let counter = new_counter();
    let counter = &counter;

    c.bench_function("get_or_load_batch/full_miss", |b| {
        b.to_async(&rt).iter(|| async {
            let start = counter.fetch_add(KEY_COUNT, Ordering::Relaxed);
            let keys: Vec<u64> = (start..start + KEY_COUNT).collect();
            let result = cache
                .get_or_load_batch(
                    &keys,
                    key_provider,
                    L2_TTL_SECS,
                    |ids| loader(ids, make_row),
                    result_mapper,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn bench_full_miss_with_db_io(c: &mut criterion::Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = redis::make_pool();
    if !rt.block_on(redis::available(&pool)) {
        eprintln!("[skip] Redis 不可用,跳过 full_miss_with_db_io");
        return;
    }

    let cache = make_cache("bench_full_miss_with_db_io", pool, true, 0);
    let counter = new_counter();
    let counter = &counter;

    c.bench_function("get_or_load_batch/full_miss_with_db_io", |b| {
        b.to_async(&rt).iter(|| async {
            let start = counter.fetch_add(KEY_COUNT, Ordering::Relaxed);
            let keys: Vec<u64> = (start..start + KEY_COUNT).collect();
            let result = cache
                .get_or_load_batch(
                    &keys,
                    key_provider,
                    L2_TTL_SECS,
                    |ids| delayed_loader(DB_IO_DELAY, ids, make_row),
                    result_mapper,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn main() {
    bench_support::run_bench(
        bench_support::redis::flush_before_bench,
        |c| {
            bench_all_l1_hits(c);
            bench_disabled(c);
            bench_disabled_with_db_io(c);
            bench_l1_miss_l2_hit(c);
            bench_full_miss(c);
            bench_full_miss_with_db_io(c);
        },
    );
}
