//! 共享基准测试基础设施
//!
//! 提供:
//! - [`criterion`]: criterion crate 重导出,bench 无需直接依赖
//! - [`criterion()`]: 统一 criterion 配置
//! - [`run_bench`]: 统一入口(setup + 场景)
//! - [`redis`]: Redis 连接池与清理(默认专用 db,避免误删本地数据)
//! - [`s3_mock`]: 嵌入式内存 S3 mock(axum)
//! - [`loader`] / [`delayed_loader`]: db mock loader(零 IO / 带固定延迟)
//! - [`new_counter`]: 时间戳递增计数器起点(避免跨运行 key 冲突)
//!
//! # 环境变量
//! - `BENCH_REDIS_URL`: Redis 地址(默认 `redis://127.0.0.1:6379/15`,专用 db)
//! - `BENCH_NO_FLUSH`: 设置为任意值可跳过 Redis 清理
//! - `BENCH_DATABASE_URL`: 保留字段(当前 DB 以 mock 方式提供)

pub use criterion;

use criterion::Criterion;

mod keys;
mod mock;
pub mod redis;
pub mod s3_mock;

pub use keys::new_counter;
pub use mock::{delayed_loader, loader};

/// 统一 criterion 配置
pub fn criterion_config() -> Criterion {
    Criterion::default().configure_from_args()
}

/// 统一基准入口:先执行 `setup`(如清理 Redis),再运行全部场景
pub fn run_bench(setup: impl FnOnce(), benches: impl FnOnce(&mut Criterion)) {
    setup();
    let mut c = criterion_config();
    benches(&mut c);
}
