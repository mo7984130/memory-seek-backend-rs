//! 事务强一致审计域。
//!
//! 审计域不创建或管理事务。调用方必须把 [`AuditService::append`] 放在自己的
//! `DatabaseTransaction` 中，审计写入失败时由调用方事务统一回滚。

use common::error::contextual::Result as ContextualResult;
use sea_orm::ConnectionTrait;
use sea_orm::DatabaseTransaction;

pub use types::audit::{
    AuditEvent, AuditQuery, AuditStatsQuery, AuditTopQuery, BehaviorRecord, BehaviorRecordReq,
};

mod mapper;

#[cfg(feature = "controller")]
pub mod controller;
#[cfg(feature = "controller")]
pub use controller::AuditController;

#[derive(Clone)]
#[cfg(feature = "controller")]
pub struct AuditState {
    pub db: sea_orm::DatabaseConnection,
}

pub struct AuditService;

impl AuditService {
    pub async fn query_behavior_stats(
        db: &impl ConnectionTrait,
        req: &AuditStatsQuery,
    ) -> ContextualResult<Vec<(chrono::DateTime<chrono::Utc>, i64)>> {
        #[cfg(not(feature = "enable"))]
        {
            let _ = (db, req);
            Ok(Vec::new())
        }
        #[cfg(feature = "enable")]
        {
            mapper::AuditMapper::query_stats(
                db,
                req.action,
                req.target_type,
                req.start,
                req.end,
                req.granularity.as_trunc(),
            )
            .await
        }
    }

    pub async fn query_behavior_top(
        db: &impl ConnectionTrait,
        req: &AuditTopQuery,
    ) -> ContextualResult<Vec<(i64, i64)>> {
        #[cfg(not(feature = "enable"))]
        {
            let _ = (db, req);
            Ok(Vec::new())
        }
        #[cfg(feature = "enable")]
        {
            mapper::AuditMapper::query_top_targets(db, req.action, req.target_type, req.limit).await
        }
    }

    pub async fn query_behavior_audit(
        db: &impl ConnectionTrait,
        req: &AuditQuery,
    ) -> ContextualResult<Vec<BehaviorRecord>> {
        #[cfg(not(feature = "enable"))]
        {
            let _ = (db, req);
            Ok(Vec::new())
        }
        #[cfg(feature = "enable")]
        {
            mapper::AuditMapper::query_audit_page(
                db,
                req.action,
                req.target_type,
                req.target_id,
                req.user_id,
                &req.cursor,
                req.size,
            )
            .await
        }
    }
}

impl AuditService {
    /// 在调用方当前事务中追加审计事实。
    pub async fn append(txn: &DatabaseTransaction, event: AuditEvent) -> common::Result<()> {
        Self::append_many(txn, [event]).await
    }

    /// 在调用方当前事务中批量追加审计事实。
    ///
    /// 多条事件通过一次批量 INSERT 写入，调用方事务仍负责保证业务数据
    /// 与审计数据的一致性。
    pub async fn append_many(
        txn: &DatabaseTransaction,
        events: impl IntoIterator<Item = AuditEvent>,
    ) -> common::Result<()> {
        let events = events.into_iter().collect::<Vec<_>>();

        #[cfg(not(feature = "enable"))]
        {
            let _ = (txn, events);
            Ok(())
        }

        #[cfg(feature = "enable")]
        {
            use common::error::{AppError, ContextualError};
            use sea_orm::{ActiveValue::Set, EntityTrait};
            use types::audit::Entity;

            if events.is_empty() {
                return Ok(());
            }

            let models = events.into_iter().map(|mut event| {
                if event.event_id == 0 {
                    event.event_id = common::snowflake::next_id();
                }
                types::audit::ActiveModel {
                    event_id: Set(event.event_id),
                    event_type: Set(event.event_type),
                    actor_id: Set(event.actor_id),
                    target_type: Set(event.target_type),
                    target_id: Set(event.target_id),
                    detail: Set(event.detail),
                    occurred_at: Set(event.occurred_at),
                }
            });

            Entity::insert_many(models)
                .exec(txn)
                .await
                .map_err(|error| {
                    ContextualError::error(
                        "audit_event_insert_many",
                        "批量写入审计事件失败",
                        error,
                        AppError::InternalServerError,
                    )
                })?;
            Ok(())
        }
    }
}
