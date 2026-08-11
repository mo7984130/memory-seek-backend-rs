//! `MultiLevelCache::get_or_load_batch` 基准测试
//!
//! 只保留带固定 DB 延迟的场景(现实查询总有数据库耗时),无 DB 延迟的纯 CPU
//! 路径不再测量。
//!
//! 覆盖路径:
//! - `disabled_with_db_io`: 缓存整体禁用,直接穿透带固定延迟的 loader
//! - `full_miss_with_db_io`: 全 miss,穿透带固定延迟的 loader 并回填 L1/L2
//!
//! L2 后端使用 `bench_support::cache::MockRedis`(纯内存,零网络 IO),
//! 消除真实 Redis 往返对测量的波动;loader 内置固定 [`DB_IO_DELAY`] 延迟,
//! 结果稳定可复现。
//!
//! 运行: `cargo bench -p common --bench get_or_load_batch`
//! 完整报告(criterion HTML)位于 `target/criterion/report/index.html`。

use std::sync::atomic::Ordering;
use std::time::Duration;

use bench_support::cache::{MockRedis, make_cache};
use bench_support::{criterion, id_of, key_of, keys, mock, new_counter};

const KEY_COUNT: u64 = 100;
const L2_TTL_SECS: u64 = 3600;
/// 模拟一次数据库查询的耗时（与真实 DB 往返量级对齐）
const DB_IO_DELAY: Duration = Duration::from_millis(5);

fn bench_disabled_with_db_io(c: &mut criterion::Criterion) {
    let cache = make_cache::<mock::MockRow>("bench_disabled_with_db_io", MockRedis::new(), false, 0);
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

    let cache = make_cache::<mock::MockRow>("bench_full_miss_with_db_io", MockRedis::new(), true, 0);
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
            bench_disabled_with_db_io(c);
            bench_full_miss_with_db_io(c);
        },
    );
}
