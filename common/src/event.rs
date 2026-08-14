//! 进程内后台领域事件。

use std::sync::Arc;

use async_trait::async_trait;

use crate::Result;

/// 领域事件订阅者。事件与状态在同一进程内通过 [`dispatch_background`] 异步分发。
#[async_trait]
pub trait EventHandler<State, Event>: Send + Sync {
    /// 订阅者名称，用于日志和监控定位。
    fn name(&self) -> &'static str;

    /// 处理事件。错误仅记录，不影响已提交的业务操作。
    async fn on_event(&self, state: Arc<State>, event: Arc<Event>) -> Result<()>;
}

/// 在后台将事件分发给全部订阅者。
///
/// 这是进程内、非持久化的最终一致性机制；每个订阅者独立运行，单个失败不影响其他订阅者。
pub fn dispatch_background<State, Event>(
    event_name: &'static str,
    state: Arc<State>,
    event: Event,
    handlers: &'static [&'static dyn EventHandler<State, Event>],
) where
    State: Send + Sync + 'static,
    Event: Send + Sync + 'static,
{
    let event = Arc::new(event);
    for handler in handlers {
        let state = Arc::clone(&state);
        let event = Arc::clone(&event);
        tokio::spawn(async move {
            if let Err(error) = handler.on_event(state, event).await {
                crate::caller_warn!(
                    %event_name,
                    handler = handler.name(),
                    ?error,
                    "background_event_failed"
                );
            }
        });
    }
}
