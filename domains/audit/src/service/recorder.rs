use common::error::contextual::Result;
use sea_orm::DatabaseTransaction;
use types::audit::AuditEvent;

pub struct AuditRecorder;

impl AuditRecorder {
    /// 在调用方当前事务中追加审计事实。
    pub async fn append(txn: &DatabaseTransaction, event: AuditEvent) -> Result<()> {
        Self::append_many(txn, [event]).await
    }

    /// 在调用方当前事务中批量追加审计事实。
    ///
    /// 多条事件通过一次批量 INSERT 写入，调用方事务仍负责保证业务数据
    /// 与审计数据的一致性。
    #[cfg(not(feature = "recording"))]
    pub async fn append_many<I>(txn: &DatabaseTransaction, events: I) -> Result<()>
    where
        I: IntoIterator<Item = AuditEvent>,
        I::IntoIter: ExactSizeIterator,
    {
        let _ = (txn, events);
        Ok(())
    }

    /// 多条事件通过一次批量 INSERT 写入，调用方事务仍负责保证业务数据
    /// 与审计数据的一致性。
    #[cfg(feature = "recording")]
    pub async fn append_many<I>(txn: &DatabaseTransaction, events: I) -> Result<()>
    where
        I: IntoIterator<Item = AuditEvent>,
        I::IntoIter: ExactSizeIterator,
    {
        use sea_orm::{ActiveValue::Set, EntityTrait};
        use types::audit::ActiveModel;
        use types::audit::Entity;

        let models = events.into_iter().map(|mut event| {
            use types::audit::AuditId;

            if event.event_id == AuditId(0) {
                event.event_id = AuditId(common::utils::snowflake::next_id());
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

        Entity::insert_many(models).exec(txn).await?;
        Ok(())
    }
}
