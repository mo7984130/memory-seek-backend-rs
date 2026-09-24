use crate::error::BackupError;
use crate::exporter::BinaryCopyExporter;
use crate::importer::BinaryCopyImporter;
use crate::manifest::BackupManifest;
use crate::state::BackupState;
use crate::storage::{BackupSource, BackupTier};
use audit::{AuditEvent, AuditRecorder};
use chrono::{Datelike, Local, Weekday};
use common_core::Result;
use common_core::ext::ToOk;
use common_core::time::{Duration, now};
use common_db::utils::table_metadata::TableMetadata;
use common_metrics::inc_counter;
use serde_json::json;
use std::sync::Arc;
use types_core::AdminId;

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

    /// 本次备份应写入的 tier 列表。
    ///
    /// - `Scheduled`: 每天写入 Daily;每周第一天(周一)追加 Weekly;每月 1 号追加 Monthly。
    ///   保证 weekly / monthly 目录里保留的是对应周期的 run,而非最近若干天。
    /// - `Manual`: 仅写入 Manual。
    fn tiers(self, now: &chrono::DateTime<Local>) -> Vec<BackupTier> {
        match self {
            Self::Manual => vec![BackupTier::Manual],
            Self::Scheduled => Self::scheduled_tiers(now.weekday(), now.day()),
        }
    }

    /// GFS 分层选择:周一 = Weekly,每月 1 号 = Monthly(周一且恰逢 1 号时两者都写)。
    fn scheduled_tiers(weekday: Weekday, day_of_month: u32) -> Vec<BackupTier> {
        let mut tiers = vec![BackupTier::Daily];
        if weekday == Weekday::Mon {
            tiers.push(BackupTier::Weekly);
        }
        if day_of_month == 1 {
            tiers.push(BackupTier::Monthly);
        }
        tiers
    }
}

impl BackupService {
    /// 从指定备份恢复数据。
    #[common_macros::metered(name = "restore")]
    #[tracing::instrument(name = "restore", skip_all, fields(run_id = %run_id))]
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
    /// 执行定时备份，并在完成后清理过期备份（无论备份成败都会清理）。
    #[common_macros::metered(name = "scheduled")]
    #[tracing::instrument(name = "scheduled", skip_all)]
    pub async fn execute_scheduled(state: Arc<BackupState>) -> Result<BackupResult> {
        let tables = Self::configured_tables(&state).await?;
        // 备份失败不能短路清理：归档可能已部分上传到存储，过期 run 仍需要收敛
        let mut result = match Self::execute(state.clone(), tables, BackupMode::Scheduled).await {
            Ok(result) => result,
            Err(error) => {
                tracing::error!(error = %error, "定时备份执行失败");
                if let Err(cleanup_error) = Self::run_cleanup(&state).await {
                    tracing::error!(error = %cleanup_error, "GFS 清理失败");
                }
                return Err(error.into());
            }
        };

        // 清理是任务的一部分：清理失败视为本次定时任务失败，向上返回错误
        result.cleaned = Self::run_cleanup(&state).await?;

        Self::record_run_audit(&state, "backup.scheduled_completed", None, &result).await?;
        inc_counter!("scheduled", "tables_exported", result.exported as u64);
        inc_counter!("scheduled", "tables_failed", result.failed as u64);
        inc_counter!("scheduled", "cleaned", result.cleaned as u64);
        Ok(result)
    }

    /// 执行 GFS 分层清理，错误向上返回，由调用方决定任务成败。
    async fn run_cleanup(state: &BackupState) -> std::result::Result<u32, BackupError> {
        state.storage.cleanup_gfs(&state.config.scheduled).await
    }

    /// 执行管理员触发的全表手动备份。
    #[common_macros::metered(name = "manual")]
    #[tracing::instrument(name = "manual", skip_all)]
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
        // tier 选择基于本地日期(周一/每月 1 号),与调度时刻的时区一致
        let backup_time = Local::now();
        let work_dir = state.temp_dir.join(&run_id);
        state.ensure_dirs()?;

        tracing::info!(run_id = %run_id, mode = %mode.metric_scope(), "开始备份");
        let mut result = BackupResult::new(run_id.clone());

        for table_name in tables {
            match BinaryCopyExporter::export_to_dir(&state.db, &table_name, &work_dir).await {
                Ok(archive_path) => {
                    let save_result = async {
                        for tier in mode.tiers(&backup_time) {
                            state
                                .storage
                                .save(&table_name, &archive_path, tier, &run_id)
                                .await?;
                        }
                        Ok::<(), BackupError>(())
                    }
                    .await;

                    match save_result {
                        Ok(()) => result.exported += 1,
                        Err(error) => {
                            result.failed += 1;
                            tracing::error!(run_id = %run_id, table = %table_name, error = %error, "保存备份失败");
                        }
                    }
                    std::fs::remove_file(&archive_path).map_err(BackupError::from)?;
                }
                Err(error) => {
                    result.failed += 1;
                    tracing::error!(run_id = %run_id, table = %table_name, error = %error, "导出备份失败");
                }
            }
        }

        if result.failed == 0 {
            let postgres_major = TableMetadata::postgres_major_version(&state.db).await?;
            let manifest =
                BackupManifest::new(result.run_id.clone(), manifest_tables, postgres_major);
            for tier in mode.tiers(&backup_time) {
                state
                    .storage
                    .save_manifest(tier, &result.run_id, &manifest)
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
        actor_id: Option<types_core::UserId>,
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

        common_db::db_transaction!(scoped & state.db, |txn| {
            AuditRecorder::append(txn, event).await?;
            Ok(())
        })
        .await?
        .to_ok()
    }

    async fn record_restore_audit(state: &BackupState, event: AuditEvent) -> Result<()> {
        common_db::db_transaction!(scoped & state.db, |txn| {
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

#[cfg(test)]
mod tests {
    use super::BackupMode;
    use chrono::Weekday;
    use types_backup::BackupTier;

    #[test]
    fn scheduled_backup_stays_daily_on_regular_days() {
        assert_eq!(
            BackupMode::scheduled_tiers(Weekday::Tue, 15),
            vec![BackupTier::Daily]
        );
    }

    #[test]
    fn weekly_tier_on_first_day_of_week() {
        assert_eq!(
            BackupMode::scheduled_tiers(Weekday::Mon, 15),
            vec![BackupTier::Daily, BackupTier::Weekly]
        );
    }

    #[test]
    fn monthly_tier_on_first_day_of_month() {
        assert_eq!(
            BackupMode::scheduled_tiers(Weekday::Tue, 1),
            vec![BackupTier::Daily, BackupTier::Monthly]
        );
    }

    #[test]
    fn month_starting_on_monday_gets_all_scheduled_tiers() {
        assert_eq!(
            BackupMode::scheduled_tiers(Weekday::Mon, 1),
            vec![BackupTier::Daily, BackupTier::Weekly, BackupTier::Monthly]
        );
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
