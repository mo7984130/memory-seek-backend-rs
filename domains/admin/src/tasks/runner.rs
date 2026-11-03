use std::sync::Arc;
use std::time::Duration;

use tokio::time::{MissedTickBehavior, timeout};
use tokio_util::sync::CancellationToken;
use types_admin::job::AdminJobStatus;
use types_admin::job_entity::Model;

use crate::AdminState;
use crate::tasks::{dispatcher::Dispatcher, repo, repo::HeartbeatOutcome};

/// 优雅关闭时等待执行器收尾的上限。
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

/// 单个任务的执行生命周期：执行、续租、轮询取消、结果落库。
pub struct Runner;

impl Runner {
    /// 执行一个已经由 supervisor 领取的任务。
    pub async fn run(state: Arc<AdminState>, job: Model, shutdown: CancellationToken) {
        let Some(run_id) = job.run_id.clone() else {
            return;
        };

        let job_token = CancellationToken::new();
        let mut execution = Box::pin(Dispatcher::execute(state.clone(), &job, job_token.clone()));

        let mut heartbeat = tokio::time::interval(state.jobs.heartbeat_interval());
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut cancel_seen = false;

        loop {
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => {
                    job_token.cancel();
                    match timeout(SHUTDOWN_GRACE, &mut execution).await {
                        Ok(Ok(value)) => {
                            Self::finish(&state, &job, &run_id, AdminJobStatus::Succeeded, Some(value), None).await;
                        }
                        Ok(Err(error)) if cancel_seen => {
                            Self::finish(&state, &job, &run_id, AdminJobStatus::Cancelled, None, Some(error.to_string())).await;
                        }
                        Ok(Err(error)) => {
                            // 关闭信号打断了执行：交还队列，让其它实例立即接手。
                            Self::release(&state, &job, &run_id).await;
                            tracing::info!(
                                job_id = %job.id,
                                error = %error,
                                "任务因实例关闭被中断，已交还队列"
                            );
                        }
                        Err(_) => {
                            // 执行器未在宽限期内退出：停止心跳，交给失联回收标记失败，
                            // 避免新旧执行者同时操作。
                            tracing::warn!(
                                job_id = %job.id,
                                "任务未在关闭宽限期内退出，等待心跳超时后回收"
                            );
                        }
                    }
                    return;
                }
                _ = heartbeat.tick() => {
                    match repo::heartbeat(&state.db, &job.id, &run_id).await {
                        Ok(HeartbeatOutcome::Alive) => {}
                        Ok(HeartbeatOutcome::CancelRequested) => {
                            if !cancel_seen {
                                cancel_seen = true;
                                job_token.cancel();
                            }
                        }
                        Ok(HeartbeatOutcome::Lost) => {
                            // 任务已被回收或放弃：立即停止，且不再写入结果。
                            job_token.cancel();
                            tracing::warn!(job_id = %job.id, "任务已不再由当前执行者持有，放弃本地执行");
                            return;
                        }
                        Err(error) => {
                            error.emit();
                        }
                    }
                }
                result = &mut execution => {
                    let (status, result, error) = if cancel_seen {
                        (
                            AdminJobStatus::Cancelled,
                            None,
                            Some("cancelled".to_owned()),
                        )
                    } else {
                        match result {
                            Ok(value) => (AdminJobStatus::Succeeded, Some(value), None),
                            Err(error) => (AdminJobStatus::Failed, None, Some(error.to_string())),
                        }
                    };
                    Self::finish(&state, &job, &run_id, status, result, error).await;
                    return;
                }
            }
        }
    }

    /// 写入终态；`run_id` 不匹配时结果会被数据库丢弃。
    async fn finish(
        state: &AdminState,
        job: &Model,
        run_id: &str,
        status: AdminJobStatus,
        result: Option<serde_json::Value>,
        error: Option<String>,
    ) {
        match repo::finish(&state.db, &job.id, run_id, status, result, error).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(job_id = %job.id, "任务结果写入被拒绝：任务已不再由当前执行者持有");
            }
            Err(error) => {
                error.emit();
            }
        }
    }

    /// 交还队列，让其它实例接手。
    async fn release(state: &AdminState, job: &Model, run_id: &str) {
        match repo::release(&state.db, &job.id, run_id).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(job_id = %job.id, "任务释放失败：任务已不再由当前执行者持有");
            }
            Err(error) => {
                error.emit();
            }
        }
    }
}
