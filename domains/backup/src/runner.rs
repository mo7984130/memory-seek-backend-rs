use crate::error::BackupError;
use crate::exporter::CsvExporter;
use crate::hasher::TableHasher;
use crate::state::BackupState;
use crate::storage::BackupType;
use crate::storage::CleanupIssueSeverity;
use audit::{AuditEvent, AuditService};
use common::{Result, caller_error, caller_warn, inc_counter, inc_error};
use serde_json::json;
use std::sync::Arc;
use types::auth::user::AdminId;

/// 备份执行器
pub struct BackupRunner;

impl BackupRunner {
    /// 获取需要备份的表名列表
    async fn get_tables(state: &BackupState) -> std::result::Result<Vec<String>, BackupError> {
        if let Some(ref tables) = state.config.tables {
            return Ok(tables.clone());
        }
        TableHasher::get_all_tables(&state.db).await
    }

    /// 定时调度备份：导出并保存到 daily / weekly / monthly，然后 GFS 清理
    #[common::metered(name = "scheduled")]
    pub async fn execute_scheduled(state: Arc<BackupState>) -> Result<BackupResult> {
        let start = std::time::Instant::now();
        let run_id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();

        tracing::info!(run_id = %run_id, "Starting scheduled backup");

        state.ensure_dirs().map_err(BackupError::from)?;

        let tables = Self::get_tables(&state).await?;
        let mut result = BackupResult::default();

        for table_name in tables {
            match Self::export_table(&state, &table_name).await {
                Ok(csv_path) => {
                    match state
                        .storage
                        .save_scheduled_all(&table_name, &csv_path, &run_id)
                        .await
                    {
                        Ok(_) => {
                            result.exported += 1;
                            tracing::info!(run_id = %run_id, table = %table_name, "Table backed up (scheduled)");
                        }
                        Err(e) => {
                            result.failed += 1;
                            inc_error!("scheduled", "save");
                            caller_error!(run_id = %run_id, table = %table_name, error = %e, "Save failed");
                        }
                    }
                    let _ = std::fs::remove_file(&csv_path);
                }
                Err(e) => {
                    result.failed += 1;
                    inc_error!("scheduled", "export");
                    caller_error!(run_id = %run_id, table = %table_name, error = %e, "Export failed");
                }
            }
        }

        // GFS 清理
        match state.storage.cleanup_gfs(&state.config.scheduled).await {
            Ok(report) => {
                result.cleaned = report.removed;
                for issue in report.issues {
                    match issue.severity {
                        CleanupIssueSeverity::Error => caller_error!(
                            run = %issue.run_id,
                            dir = %issue.rel_dir,
                            error = %issue.error,
                            "Failed to remove local backup dir"
                        ),
                        CleanupIssueSeverity::Warn => caller_warn!(
                            run = %issue.run_id,
                            dir = %issue.rel_dir,
                            error = %issue.error,
                            "GFS cleanup partial S3 deletion"
                        ),
                    }
                }
                tracing::info!("GFS cleanup: removed {} expired backups", result.cleaned);
            }
            Err(e) => {
                inc_error!("scheduled", "cleanup");
                caller_error!(error = %e, "GFS cleanup failed");
            }
        }

        result.duration = start.elapsed();
        Self::record_run_audit(&state, "backup.scheduled_completed", None, &run_id, &result)
            .await?;
        inc_counter!("scheduled", "tables_exported", result.exported as u64);
        inc_counter!("scheduled", "tables_failed", result.failed as u64);
        inc_counter!("scheduled", "cleaned", result.cleaned as u64);
        tracing::info!(
            "Scheduled backup completed in {:?}: {} exported, {} failed, {} cleaned",
            result.duration,
            result.exported,
            result.failed,
            result.cleaned
        );

        Ok(result)
    }

    /// 手动备份：导出并保存到 manual 目录（永不清理）
    #[common::metered(name = "manual")]
    pub async fn execute_manual(state: Arc<BackupState>, admin: AdminId) -> Result<BackupResult> {
        let start = std::time::Instant::now();
        let run_id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let admin_id = admin.into_inner();

        tracing::info!(
            run_id = %run_id,
            user_id = %admin_id,
            "Starting manual backup"
        );

        state.ensure_dirs().map_err(BackupError::from)?;

        let tables = Self::get_tables(&state).await?;
        let mut result = BackupResult::default();

        for table_name in tables {
            match Self::export_table(&state, &table_name).await {
                Ok(csv_path) => {
                    match state
                        .storage
                        .save(&table_name, &csv_path, BackupType::Manual, &run_id)
                        .await
                    {
                        Ok(_) => {
                            result.exported += 1;
                            tracing::info!(run_id = %run_id, table = %table_name, "Table backed up (manual)");
                        }
                        Err(e) => {
                            result.failed += 1;
                            inc_error!("manual", "save");
                            caller_error!(run_id = %run_id, table = %table_name, error = %e, "Manual save failed");
                        }
                    }
                    let _ = std::fs::remove_file(&csv_path);
                }
                Err(e) => {
                    result.failed += 1;
                    inc_error!("manual", "export");
                    caller_error!(run_id = %run_id, table = %table_name, error = %e, "Export failed");
                }
            }
        }

        result.duration = start.elapsed();
        Self::record_run_audit(
            &state,
            "backup.manual_completed",
            Some(admin_id),
            &run_id,
            &result,
        )
        .await?;
        inc_counter!("manual", "tables_exported", result.exported as u64);
        inc_counter!("manual", "tables_failed", result.failed as u64);
        tracing::info!(
            "Manual backup completed in {:?}: {} exported, {} failed",
            result.duration,
            result.exported,
            result.failed
        );

        Ok(result)
    }

    /// 导出单张表到临时目录，返回 CSV 文件路径
    async fn export_table(
        state: &BackupState,
        table_name: &str,
    ) -> std::result::Result<std::path::PathBuf, BackupError> {
        let (csv_path, _) = CsvExporter::export(&state.db, table_name, &state.temp_dir).await?;
        Ok(csv_path)
    }

    /// 在短事务中记录一次备份运行结果；不把文件导出和 S3 操作放进数据库事务。
    async fn record_run_audit(
        state: &BackupState,
        event_type: &str,
        actor_id: Option<types::auth::user::UserId>,
        run_id: &str,
        result: &BackupResult,
    ) -> Result<()> {
        let event = AuditEvent::new(event_type).with_detail(json!({
            "run_id": run_id,
            "exported": result.exported,
            "failed": result.failed,
            "cleaned": result.cleaned,
            "duration_ms": result.duration.as_millis(),
        }));
        let event = actor_id.map_or(event.clone(), |actor_id| event.with_actor(actor_id.0));

        common::db_transaction!(scoped & state.db, |txn| {
            AuditService::append(txn, event).await
        })
        .await
    }
}

#[derive(Debug, Default)]
pub struct BackupResult {
    pub exported: u32,
    pub failed: u32,
    pub cleaned: u32,
    pub duration: std::time::Duration,
}
