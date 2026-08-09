//! 多级缓存组件
//!
//! 统一项目中读多写少热点数据的缓存使用，分为三级：
//! - L1: 进程内 moka 缓存（直接存储反序列化对象，命中零拷贝）
//! - L2: Redis 分布式缓存（JSON 序列化存储）
//! - L3: 数据库（最终数据源，由调用方提供的 loader 承担）
//!
//! 写操作采用「覆盖 + 删除」策略：更新数据时通过 [`MultiLevelCache::put`] 直接覆盖
//! L1 与 L2，避免先删后写带来的穿透窗口；删除数据时通过 [`MultiLevelCache::invalidate`]
//! 同时清理 L1 与 L2。
//!
//! 本地缓存（L1）仅依赖短 TTL 与主动失效保证最终一致，多实例场景下不做跨实例广播。
//!
//! # 监控
//!
//! 启用 `metrics` feature 时，每个缓存实例按 `cache:{name}:{layer}:{op}` 命名采集
//! 命中率、耗时与容量指标。

mod multi_level;

pub use multi_level::MultiLevelCache;
