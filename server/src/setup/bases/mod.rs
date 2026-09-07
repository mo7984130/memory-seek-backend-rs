pub mod database;
pub mod log;
pub mod redis;
pub mod task_manager;

#[cfg(feature = "_cache")]
pub mod cache;

#[cfg(feature = "metrics")]
pub mod metrics;

#[linkme::distributed_slice]
pub static APP_BASES: [crate::setup::InitFn];
#[linkme::distributed_slice]
pub static APP_BASES_LAST: [crate::setup::InitFn];
