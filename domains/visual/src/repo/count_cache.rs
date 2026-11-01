use std::{future::Future, time::Duration};

use common_core::{
    error::contextual::Result,
    error::contextual::ext::{ContextualResultExt, IntoContextualExt},
};
use common_metrics::{MetricsTimerExt, metrics_name};

use crate::VisualState;

/// 列表总量比影像详情短缓存，避免分页期间重复执行聚合查询。
pub(crate) const CURSOR_COUNT_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

pub(crate) async fn get_or_load<F, Fut>(state: &VisualState, key: &str, loader: F) -> Result<u64>
where
    F: FnOnce() -> Fut + Send,
    Fut: Future<Output = Result<u64>> + Send,
{
    state
        .cache_visual_counts
        .get_or_load(key, CURSOR_COUNT_CACHE_TTL, loader)
        .timed(metrics_name!("cache_get_or_load_cursor_count"))
        .await
}

pub(crate) async fn invalidate(state: &VisualState, key: &str) {
    state
        .cache_visual_counts
        .invalidate(key)
        .timed(metrics_name!("cache_invalidate_cursor_count"))
        .await
        .into_contextual()
        .emit_if_err();
}
