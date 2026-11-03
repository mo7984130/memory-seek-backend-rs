use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::time::MissedTickBehavior;

use crate::AdminState;
use crate::tasks::{repo, runner::Runner};

/// 启动后台任务调度器；每个实例各跑一份，依靠数据库协调。
pub fn start(state: Arc<AdminState>) {
    let supervisor_state = state.clone();
    state
        .task_manager
        .spawn("admin_job_supervisor", move |shutdown| {
            let state = supervisor_state;
            async move {
                let mut ticker = tokio::time::interval(state.jobs.poll_interval());
                ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
                let inflight = Arc::new(AtomicUsize::new(0));

                loop {
                    tokio::select! {
                        biased;
                        _ = shutdown.cancelled() => break,
                        _ = ticker.tick() => tick(&state, &inflight).await,
                    _ = state.job_wake.notified() => tick(&state, &inflight).await,
                    }
                }
            }
        });
}

/// 一轮调度：回收心跳中断的任务、收敛取消请求，再尽可能领取新任务。
async fn tick(state: &Arc<AdminState>, inflight: &Arc<AtomicUsize>) {
    match repo::sweep_queued_cancellations(&state.db).await {
        Ok(0) => {}
        Ok(count) => tracing::info!(count, "已收敛取消的排队任务"),
        Err(error) => {
            error.emit();
        }
    }

    match repo::reap_stale(&state.db, state.jobs.stale_after()).await {
        Ok(0) => {}
        Ok(count) => tracing::warn!(count, "已回收心跳中断的任务"),
        Err(error) => {
            error.emit();
        }
    }

    while inflight.load(Ordering::Relaxed) < state.jobs.max_concurrency() {
        match repo::claim_next(&state.db, &state.instance_id).await {
            Ok(Some(job)) => {
                inflight.fetch_add(1, Ordering::Relaxed);
                spawn_runner(state.clone(), job, InflightGuard(inflight.clone()));
            }
            Ok(None) => break,
            Err(error) => {
                error.emit();
                break;
            }
        }
    }
}

/// 为单个任务启动执行器；`guard` 在任务结束时归还并发额度。
fn spawn_runner(state: Arc<AdminState>, job: types_admin::job_entity::Model, guard: InflightGuard) {
    let task_manager = state.task_manager.clone();
    task_manager.spawn("admin_job_runner", move |shutdown| {
        let state = state;
        async move {
            let _guard = guard;
            Runner::run(state, job, shutdown).await;
        }
    });
}

/// 单实例本地并发额度；仅在进程内生效，不承担任务状态语义。
struct InflightGuard(Arc<AtomicUsize>);

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
