use crate::error::BackupError;
use crate::exporter::BinaryCopyExporter;
use crate::importer::BinaryCopyImporter;
use crate::manifest::BackupManifest;
use crate::state::BackupState;
use crate::storage::{BackupSource, BackupTier};
use audit::{AuditEvent, AuditRecorder};
use common::ext::ToOk;
use common::time::{Duration, now};
use common::utils::table_metadata::TableMetadata;
use common::{Result, inc_counter, inc_error};
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
    pub async fn restore(
        state: Arc<BackupState>,
        admin: AdminId,
        source: BackupSource,
        tier: BackupTier,
        run_id: String,
        confirm_run_id: String,
    ) -> Result<u64> {
        if run_id != confirm_run_id {
            return Err(BackupError::Msg("恢复确认 ID 不匹配".to_string()).into());
        }
        let _guard = state.operation_lock.lock().await;
        let manifest = state.storage.load_manifest(source, tier, &run_id).await?;
        if manifest.run_id != run_id || manifest.format_version != crate::manifest::FORMAT_VERSION {
            return Err(BackupError::Msg("备份清单无效或格式不兼容".to_string()).into());
        }
        let postgres_major = TableMetadata::postgres_major_version(&state.db)
            .await
            .map_err(BackupError::from)?;
        if manifest.postgres_major != postgres_major {
            return Err(BackupError::Msg("备份与目标 PostgreSQL 主版本不兼容".to_string()).into());
        }
        if !Self::is_safe_segment(&run_id)
            || manifest.tables.is_empty()
            || !manifest
                .tables
                .iter()
                .all(|table| Self::is_safe_segment(table))
        {
            return Err(BackupError::Msg("备份清单包含非法路径片段".to_string()).into());
        }
        let tables = manifest.tables.clone();
        let restore_dir = state.temp_dir.join(format!("restore-{run_id}"));
        if matches!(source, BackupSource::S3) {
            tokio::fs::create_dir_all(&restore_dir)
                .await
                .map_err(BackupError::from)?;
            for table in &tables {
                use futures_util::TryStreamExt;
                use tokio::io::AsyncWriteExt;
                let mut stream = state
                    .storage
                    .s3_archive_stream(tier, &run_id, table)
                    .await?;
                let path = restore_dir.join(format!("{table}.copy.zst"));
                let mut file = tokio::fs::File::create(path)
                    .await
                    .map_err(BackupError::from)?;
                while let Some(chunk) = stream.try_next().await.map_err(BackupError::from)? {
                    file.write_all(&chunk).await.map_err(BackupError::from)?;
                }
                file.flush().await.map_err(BackupError::from)?;
            }
        }
        let result = BinaryCopyImporter::restore_local(&state.db, &tables, |table| match source {
            BackupSource::Local => state.storage.local_archive_path(tier, &run_id, table),
            BackupSource::S3 => restore_dir.join(format!("{table}.copy.zst")),
        })
        .await;
        let _ = tokio::fs::remove_dir_all(&restore_dir).await;
        let restored = result?;
        let event = AuditEvent::new("backup.restored")
            .with_detail(json!({
                "run_id": run_id, "tables": tables, "rows": restored,
            }))
            .with_actor(admin.into_inner().0);
        Self::record_restore_audit(&state, event).await?;
        inc_counter!("restore", "tables", manifest.tables.len() as u64);
        inc_counter!("restore", "rows", restored);
        Ok(restored)
    }
    /// 执行定时备份，并在完成后清理过期备份。
    #[common_macros::metered(name = "scheduled")]
    pub async fn execute_scheduled(state: Arc<BackupState>) -> Result<BackupResult> {
        let tables = Self::configured_tables(&state).await?;
        let mut result = Self::execute(state.clone(), tables, BackupMode::Scheduled).await?;

        match state.storage.cleanup_gfs(&state.config.scheduled).await {
            Ok(removed) => result.cleaned = removed,
            Err(error) => {
                inc_error!("scheduled", "cleanup");
                tracing::error!(error = %error, "GFS 清理失败");
            }
        }

        Self::record_run_audit(&state, "backup.scheduled_completed", None, &result).await?;
        inc_counter!("scheduled", "tables_exported", result.exported as u64);
        inc_counter!("scheduled", "tables_failed", result.failed as u64);
        inc_counter!("scheduled", "cleaned", result.cleaned as u64);
        Ok(result)
    }

    /// 执行管理员触发的全表手动备份。
    #[common_macros::metered(name = "manual")]
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
        let manifest_tables = tables.clone();
        let start = std::time::Instant::now();
        let run_id = now().format("%Y%m%d_%H%M%S").to_string();
        let work_dir = state.temp_dir.join(&run_id);
        state.ensure_dirs()?;

        tracing::info!(run_id = %run_id, mode = %mode.metric_scope(), "开始备份");
        let mut result = BackupResult::new(run_id.clone());

        for table_name in tables {
            match BinaryCopyExporter::export_to_dir(&state.db, &table_name, &work_dir).await {
                Ok(archive_path) => {
                    let save_result = async {
                        for tier in mode.tiers() {
                            state
                                .storage
                                .save(&table_name, &archive_path, *tier, &run_id)
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
                            tracing::error!(run_id = %run_id, table = %table_name, error = %error, "保存备份失败");
                        }
                    }
                    std::fs::remove_file(&archive_path).map_err(BackupError::from)?;
                }
                Err(error) => {
                    result.failed += 1;
                    match mode {
                        BackupMode::Scheduled => inc_error!("scheduled", "export"),
                        BackupMode::Manual => inc_error!("manual", "export"),
                    }
                    tracing::error!(run_id = %run_id, table = %table_name, error = %error, "导出备份失败");
                }
            }
        }

        if result.failed == 0 {
            let postgres_major = TableMetadata::postgres_major_version(&state.db).await?;
            let manifest =
                BackupManifest::new(result.run_id.clone(), manifest_tables, postgres_major);
            for tier in mode.tiers() {
                state
                    .storage
                    .save_manifest(*tier, &result.run_id, &manifest)
                    .await?;
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
            AuditRecorder::append(txn, event).await?;
            Ok(())
        })
        .await?
        .to_ok()
    }

    async fn record_restore_audit(state: &BackupState, event: AuditEvent) -> Result<()> {
        common::db_transaction!(scoped & state.db, |txn| {
            AuditRecorder::append(txn, event).await?;
            Ok(())
        })
        .await?
        .to_ok()
    }

    fn is_safe_segment(value: &str) -> bool {
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
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
