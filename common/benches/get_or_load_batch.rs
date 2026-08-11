//! `MultiLevelCache::get_or_load_batch` 基准测试
//!
//! 覆盖四条路径(有 DB 的场景必定带固定 DB 延迟,命中场景不经 DB):
//! - `all_l1_hits`: L1 全命中(热路径,不经 DB)
//! - `l1_miss_l2_hit`: 容量 0 强制 L1 miss,L2 命中(不经 DB)
//! - `disabled_with_db_io`: 缓存整体禁用,穿透带固定延迟的 loader(DB)
//! - `full_miss_with_db_io`: 全 miss,穿透带固定延迟的 loader 并回填 L1/L2(DB)
//!
//! L2 后端使用 `bench_support::cache::MockRedis`(纯内存,零网络 IO),
//! 消除真实 Redis 往返对测量的波动;DB 场景使用 [`MockDb`] 固定延迟,
//! 结果稳定可复现。
//!
//! 运行: `cargo bench -p common --bench get_or_load_batch`
//! 完整报告(criterion HTML)位于 `target/criterion/report/index.html`。

use std::sync::atomic::Ordering;
use std::time::Duration;

use bench_support::cache::{MockRedis, make_cache};
use bench_support::{criterion, id_of, key_of, keys, loader, mock, new_counter};
use common::cache::MultiLevelCache;

const KEY_COUNT: u64 = 100;
const L2_TTL_SECS: u64 = 3600;
/// 模拟一次数据库查询的耗时（与真实 DB 往返量级对齐）
const DB_IO_DELAY: Duration = Duration::from_millis(5);

async fn prefill(cache: &MultiLevelCache<mock::MockRow>, ks: &[u64]) {
    for id in ks {
        cache
            .put(&key_of(id), mock::MockRow::new(*id), L2_TTL_SECS)
            .await
            .unwrap();
    }
}

fn bench_all_l1_hits(c: &mut criterion::Criterion) {
    let cache =
        make_cache::<mock::MockRow>("bench_all_l1_hits", MockRedis::new(), true, 1024 * 1024);
    let ks = keys(KEY_COUNT);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(prefill(&cache, &ks));

    c.bench_function("get_or_load_batch/all_l1_hits", |b| {
        b.to_async(&rt).iter(|| async {
            let result = cache
                .get_or_load_batch(
                    &ks,
                    key_of,
                    L2_TTL_SECS,
                    |ids| loader(ids, mock::MockRow::new),
                    id_of,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn bench_l1_miss_l2_hit(c: &mut criterion::Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // 容量 0 强制 L1 永不命中,只预填 L2
    let cache = make_cache::<mock::MockRow>("bench_l1_miss_l2_hit", MockRedis::new(), true, 0);
    let ks = keys(KEY_COUNT);
    rt.block_on(prefill(&cache, &ks));

    c.bench_function("get_or_load_batch/l1_miss_l2_hit", |b| {
        b.to_async(&rt).iter(|| async {
            let result = cache
                .get_or_load_batch(
                    &ks,
                    key_of,
                    L2_TTL_SECS,
                    |ids| loader(ids, mock::MockRow::new),
                    id_of,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn bench_disabled_with_db_io(c: &mut criterion::Criterion) {
    let cache =
        make_cache::<mock::MockRow>("bench_disabled_with_db_io", MockRedis::new(), false, 0);
    let db = mock::MockDb::new(DB_IO_DELAY);
    let ks = keys(KEY_COUNT);
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("get_or_load_batch/disabled_with_db_io", |b| {
        b.to_async(&rt).iter(|| async {
            let result = cache
                .get_or_load_batch(
                    &ks,
                    key_of,
                    L2_TTL_SECS,
                    |ids| db.query(ids, mock::MockRow::new),
                    id_of,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn bench_full_miss_with_db_io(c: &mut criterion::Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let cache =
        make_cache::<mock::MockRow>("bench_full_miss_with_db_io", MockRedis::new(), true, 0);
    let db = mock::MockDb::new(DB_IO_DELAY);
    // 每次迭代使用全新 key 集,保证 L2 必 miss(避免回填后二次命中)
    let counter = new_counter();
    let counter = &counter;

    c.bench_function("get_or_load_batch/full_miss_with_db_io", |b| {
        b.to_async(&rt).iter(|| async {
            let start = counter.fetch_add(KEY_COUNT, Ordering::Relaxed);
            let ks: Vec<u64> = (start..start + KEY_COUNT).collect();
            let result = cache
                .get_or_load_batch(
                    &ks,
                    key_of,
                    L2_TTL_SECS,
                    |ids| db.query(ids, mock::MockRow::new),
                    id_of,
                )
                .await
                .unwrap();
            criterion::black_box(&result);
        });
    });
}

fn main() {
    bench_support::run_bench(
        || {},
        |c| {
            bench_all_l1_hits(c);
            bench_l1_miss_l2_hit(c);
            bench_disabled_with_db_io(c);
            bench_full_miss_with_db_io(c);
        },
    );
}
