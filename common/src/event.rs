//! 提交后的进程内异步事件。

use std::sync::Arc;

use async_trait::async_trait;

use crate::Result;

/// 提交后的异步事件消费者。
#[async_trait]
pub trait EventConsumer<State, Event>: Send + Sync {
    /// 消费者名称，用于日志和监控定位。
    fn name(&self) -> &'static str;

    /// 消费事件。错误仅记录，不影响已提交的业务操作。
    async fn consume(&self, state: Arc<State>, event: Arc<Event>) -> Result<()>;
}

/// 在后台将提交后的事件分发给全部消费者。
///
/// 这是进程内、非持久化的最终一致性机制；每个订阅者独立运行，单个失败不影响其他订阅者。
pub fn dispatch_async_event<State, Event>(
    event_name: &'static str,
    state: Arc<State>,
    event: Event,
    consumers: &'static [&'static dyn EventConsumer<State, Event>],
) where
    State: Send + Sync + 'static,
    Event: Send + Sync + 'static,
{
    let event = Arc::new(event);
    for consumer in consumers {
        let state = Arc::clone(&state);
        let event = Arc::clone(&event);
        tokio::spawn(async move {
            if let Err(error) = consumer.consume(state, event).await {
                crate::caller_warn!(
                    %event_name,
                    consumer = consumer.name(),
                    ?error,
                    "async_event_consume_failed"
                );
            }
        });
    }
}
