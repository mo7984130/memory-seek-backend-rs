use crate::error::BackupError;
use crate::exporter::CsvExporter;
use crate::state::BackupState;
use crate::storage::BackupTier;
use audit::{AuditEvent, AuditService};
use common::ext::ToOk;
use common::time::{Duration, now};
use common::utils::table_metadata::TableMetadata;
use common::{Result, caller_error, inc_counter, inc_error};
use serde_json::json;
use std::sync::Arc;
use types::auth::user::AdminId;

/// 备份领域用例服务。
pub struct BackupService;

#[derive(Clone, Copy)]
enum BackupMode {
    Scheduled,
    Manual,
}

impl BackupMode {
    fn metric_scope(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Manual => "manual",
        }
    }

    fn tiers(self) -> &'static [BackupTier] {
        match self {
            Self::Scheduled => &[BackupTier::Daily, BackupTier::Weekly, BackupTier::Monthly],
            Self::Manual => &[BackupTier::Manual],
        }
    }
}

impl BackupService {
    /// 执行定时备份，并在完成后清理过期备份。
    #[common::metered(name = "scheduled")]
    pub async fn execute_scheduled(state: Arc<BackupState>) -> Result<BackupResult> {
        let tables = Self::configured_tables(&state).await?;
        let mut result = Self::execute(state.clone(), tables, BackupMode::Scheduled).await?;

        match state.storage.cleanup_gfs(&state.config.scheduled).await {
            Ok(removed) => result.cleaned = removed,
            Err(error) => {
                inc_error!("scheduled", "cleanup");
                caller_error!(error = %error, "GFS 清理失败");
            }
        }

        Self::record_run_audit(&state, "backup.scheduled_completed", None, &result).await?;
        inc_counter!("scheduled", "tables_exported", result.exported as u64);
        inc_counter!("scheduled", "tables_failed", result.failed as u64);
        inc_counter!("scheduled", "cleaned", result.cleaned as u64);
        Ok(result)
    }

    /// 执行管理员触发的全表手动备份。
    #[common::metered(name = "manual")]
    pub async fn execute_manual(state: Arc<BackupState>, admin: AdminId) -> Result<BackupResult> {
        let tables = Self::configured_tables(&state).await?;
        let result = Self::execute(state.clone(), tables, BackupMode::Manual).await?;
        Self::record_run_audit(
            &state,
            "backup.manual_completed",
            Some(admin.into_inner()),
            &result,
        )
        .await?;
        inc_counter!("manual", "tables_exported", result.exported as u64);
        inc_counter!("manual", "tables_failed", result.failed as u64);
        Ok(result)
    }

    /// 执行指定表的手动备份，供领域内部流程复用。
    pub async fn backup_tables(
        state: Arc<BackupState>,
        tables: &[&str],
    ) -> std::result::Result<BackupResult, BackupError> {
        Self::execute(
            state,
            tables.iter().map(ToString::to_string).collect(),
            BackupMode::Manual,
        )
        .await?
        .to_ok()
    }

    async fn configured_tables(
        state: &BackupState,
    ) -> std::result::Result<Vec<String>, BackupError> {
        if let Some(tables) = &state.config.tables {
            return Ok(tables.clone());
        }
        TableMetadata::get_all_tables(&state.db)
            .await
            .map_err(BackupError::from)
    }

    async fn execute(
        state: Arc<BackupState>,
        tables: Vec<String>,
        mode: BackupMode,
    ) -> std::result::Result<BackupResult, BackupError> {
        let start = std::time::Instant::now();
        let run_id = now().format("%Y%m%d_%H%M%S").to_string();
        let work_dir = state.temp_dir.join(&run_id);
        state.ensure_dirs()?;

        tracing::info!(run_id = %run_id, mode = %mode.metric_scope(), "开始备份");
        let mut result = BackupResult::new(run_id.clone());

        for table_name in tables {
            match CsvExporter::export_to_dir(&state.db, &table_name, &work_dir).await {
                Ok(csv_path) => {
                    let save_result = async {
                        for tier in mode.tiers() {
                            state
                                .storage
                                .save(&table_name, &csv_path, *tier, &run_id)
                                .await?;
                        }
                        Ok::<(), BackupError>(())
                    }
                    .await;

                    match save_result {
                        Ok(()) => result.exported += 1,
                        Err(error) => {
                            result.failed += 1;
                            match mode {
                                BackupMode::Scheduled => inc_error!("scheduled", "save"),
                                BackupMode::Manual => inc_error!("manual", "save"),
                            }
                            caller_error!(run_id = %run_id, table = %table_name, error = %error, "保存备份失败");
                        }
                    }
                    std::fs::remove_file(&csv_path).map_err(BackupError::from)?;
                }
                Err(error) => {
                    result.failed += 1;
                    match mode {
                        BackupMode::Scheduled => inc_error!("scheduled", "export"),
                        BackupMode::Manual => inc_error!("manual", "export"),
                    }
                    caller_error!(run_id = %run_id, table = %table_name, error = %error, "导出备份失败");
                }
            }
        }

        std::fs::remove_dir_all(&work_dir).map_err(BackupError::from)?;
        result.duration = start.elapsed();
        tracing::info!(
            run_id = %result.run_id,
            mode = %mode.metric_scope(),
            exported = result.exported,
            failed = result.failed,
            duration = ?result.duration,
            "备份完成"
        );
        Ok(result)
    }

    async fn record_run_audit(
        state: &BackupState,
        event_type: &str,
        actor_id: Option<types::auth::user::UserId>,
        result: &BackupResult,
    ) -> Result<()> {
        let event = AuditEvent::new(event_type).with_detail(json!({
            "run_id": result.run_id,
            "exported": result.exported,
            "failed": result.failed,
            "cleaned": result.cleaned,
            "duration_ms": result.duration.as_millis(),
        }));
        let event = actor_id.map_or(event.clone(), |actor_id| event.with_actor(actor_id.0));

        common::db_transaction!(scoped & state.db, |txn| {
            AuditService::append(txn, event).await?;
            Ok(())
        })
        .await?
        .to_ok()
    }
}

#[derive(Debug)]
pub struct BackupResult {
    pub run_id: String,
    pub exported: u32,
    pub failed: u32,
    pub cleaned: u32,
    pub duration: Duration,
}

impl BackupResult {
    fn new(run_id: String) -> Self {
        Self {
            run_id,
            exported: 0,
            failed: 0,
            cleaned: 0,
            duration: Duration::ZERO,
        }
    }
}
