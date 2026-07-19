use crate::exporter::CsvExporter;
use crate::hasher::TableHasher;
use crate::state::BackupState;
use crate::storage::BackupType;
use std::sync::Arc;

/// 备份执行器
pub struct BackupRunner;

impl BackupRunner {
    /// 获取需要备份的表名列表
    async fn get_tables(
        state: &BackupState,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref tables) = state.config.tables {
            return Ok(tables.clone());
        }
        TableHasher::get_all_tables(&state.db).await
    }

    /// 定时调度备份：导出并保存到 daily / weekly / monthly，然后 GFS 清理
    pub async fn execute_scheduled(
        state: Arc<BackupState>,
    ) -> Result<BackupResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();

        tracing::info!("Starting scheduled backup");

        state.ensure_dirs()?;

        let tables = Self::get_tables(&state).await?;
        let mut result = BackupResult::default();

        for table_name in tables {
            match Self::export_table(&state, &table_name).await {
                Ok(csv_path) => {
                    match state
                        .storage
                        .save_scheduled_all(&table_name, &csv_path)
                        .await
                    {
                        Ok(_) => {
                            result.exported += 1;
                            tracing::info!("Table {} backed up (scheduled)", table_name);
                        }
                        Err(e) => {
                            result.failed += 1;
                            tracing::error!("Table {} save failed: {}", table_name, e);
                        }
                    }
                    let _ = std::fs::remove_file(&csv_path);
                }
                Err(e) => {
                    result.failed += 1;
                    tracing::error!("Table {} export failed: {}", table_name, e);
                }
            }
        }

        // GFS 清理
        match state.storage.cleanup_gfs(&state.config.scheduled).await {
            Ok(count) => {
                result.cleaned = count;
                tracing::info!("GFS cleanup: removed {} expired backups", count);
            }
            Err(e) => {
                tracing::error!("GFS cleanup failed: {}", e);
            }
        }

        result.duration = start.elapsed();
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
    pub async fn execute_manual(
        state: Arc<BackupState>,
    ) -> Result<BackupResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();

        tracing::info!("Starting manual backup");

        state.ensure_dirs()?;

        let tables = Self::get_tables(&state).await?;
        let mut result = BackupResult::default();

        for table_name in tables {
            match Self::export_table(&state, &table_name).await {
                Ok(csv_path) => {
                    match state
                        .storage
                        .save(&table_name, &csv_path, BackupType::Manual)
                        .await
                    {
                        Ok(_) => {
                            result.exported += 1;
                            tracing::info!("Table {} backed up (manual)", table_name);
                        }
                        Err(e) => {
                            result.failed += 1;
                            tracing::error!("Table {} manual save failed: {}", table_name, e);
                        }
                    }
                    let _ = std::fs::remove_file(&csv_path);
                }
                Err(e) => {
                    result.failed += 1;
                    tracing::error!("Table {} export failed: {}", table_name, e);
                }
            }
        }

        result.duration = start.elapsed();
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
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let (csv_path, _) =
            CsvExporter::export(&state.db, table_name, &state.temp_dir).await?;
        Ok(csv_path)
    }
}

#[derive(Debug, Default)]
pub struct BackupResult {
    pub exported: u32,
    pub failed: u32,
    pub cleaned: u32,
    pub duration: std::time::Duration,
}
