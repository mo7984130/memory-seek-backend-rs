use common_core::error::contextual::Result;
use sea_orm::DatabaseTransaction;
use types_audit::AuditEvent;

pub struct AuditRecorder;

impl AuditRecorder {
    /// 在调用方当前事务中追加审计事实。
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn append(txn: &DatabaseTransaction, event: AuditEvent) -> Result<()> {
        Self::insert_many(txn, [event]).await
    }

    /// 在调用方当前事务中批量追加审计事实。
    ///
    /// 多条事件通过一次批量 INSERT 写入，调用方事务仍负责保证业务数据
    /// 与审计数据的一致性。
    #[common_macros::metered]
    #[tracing::instrument(skip_all)]
    pub async fn append_many<I>(txn: &DatabaseTransaction, events: I) -> Result<()>
    where
        I: IntoIterator<Item = AuditEvent>,
        I::IntoIter: ExactSizeIterator,
    {
        Self::insert_many(txn, events).await
    }

    /// 实际写入实现。
    ///
    /// 不带操作级指标，供 `append` / `append_many` 复用，避免内部委托造成重复计数
    /// （`append` 的指标为 `audit:append:*`，`append_many` 为 `audit:append_many:*`）。
    #[cfg(not(feature = "recording"))]
    async fn insert_many<I>(txn: &DatabaseTransaction, events: I) -> Result<()>
    where
        I: IntoIterator<Item = AuditEvent>,
        I::IntoIter: ExactSizeIterator,
    {
        let _ = (txn, events);
        Ok(())
    }

    /// 实际写入实现（`recording` 打开时真正落库）。
    #[cfg(feature = "recording")]
    async fn insert_many<I>(txn: &DatabaseTransaction, events: I) -> Result<()>
    where
        I: IntoIterator<Item = AuditEvent>,
        I::IntoIter: ExactSizeIterator,
    {
        use common_metrics::MetricsTimerExt;
        use sea_orm::{ActiveValue::Set, EntityTrait};
        use types_audit::ActiveModel;
        use types_audit::Entity;

        let models = events.into_iter().map(|mut event| {
            use types_audit::AuditId;

            if event.event_id == AuditId(0) {
                event.event_id = AuditId(common_crypto::snowflake::next_id());
            }
            ActiveModel {
                event_id: Set(event.event_id),
                event_type: Set(event.event_type),
                actor_id: Set(event.actor_id),
                target_type: Set(event.target_type),
                target_id: Set(event.target_id),
                detail: Set(event.detail),
                created_at: Set(event.occurred_at),
            }
        });

        Entity::insert_many(models)
            .exec(txn)
            .timed(common_metrics::metrics_name!("db_insert"))
            .await?;
        Ok(())
    }
}
