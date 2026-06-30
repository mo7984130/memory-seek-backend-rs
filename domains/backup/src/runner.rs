use crate::exporter::CsvExporter;
use crate::hasher::TableHasher;
use crate::state::BackupState;
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

    /// 执行备份
    pub async fn execute(
        state: Arc<BackupState>,
    ) -> Result<BackupResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();

        tracing::info!("Starting daily backup for date: {}", date);

        // 确保目录存在
        state.ensure_dirs()?;

        let tables = Self::get_tables(&state).await?;
        let mut result = BackupResult::default();

        for table_name in tables {
            match Self::backup_table(&state, &table_name, &date).await {
                Ok(table_result) => {
                    match table_result {
                        TableBackupResult::Exported => {
                            result.exported += 1;
                            tracing::info!("Table {} backed up successfully", table_name);
                        }
                        TableBackupResult::Renamed => {
                            result.renamed += 1;
                            tracing::info!("Table {} unchanged, renamed backup", table_name);
                        }
                        TableBackupResult::Skipped(reason) => {
                            result.skipped += 1;
                            tracing::warn!("Table {} skipped: {}", table_name, reason);
                        }
                    }
                    result.success += 1;
                }
                Err(e) => {
                    result.failed += 1;
                    tracing::error!("Table {} backup failed: {}", table_name, e);
                    // 继续处理其他表
                }
            }
        }

        // 清理过期备份
        match state.storage.cleanup(state.config.retention_days).await {
            Ok(count) => {
                result.cleaned = count;
                tracing::info!("Cleanup: removed {} expired backups", count);
            }
            Err(e) => {
                tracing::error!("Cleanup failed: {}", e);
            }
        }

        result.duration = start.elapsed();
        tracing::info!(
            "Daily backup completed in {:?}: {} exported, {} renamed, {} skipped, {} failed",
            result.duration,
            result.exported,
            result.renamed,
            result.skipped,
            result.failed
        );

        Ok(result)
    }

    /// 备份单个表
    async fn backup_table(
        state: &BackupState,
        table_name: &str,
        date: &str,
    ) -> Result<TableBackupResult, Box<dyn std::error::Error + Send + Sync>> {
        // 1. 计算表哈希
        let current_hash = TableHasher::compute(&state.db, table_name).await?;

        // 2. 获取上次哈希
        let last_hash = state.last_hashes.read().await.get(table_name).cloned();

        // 3. 判断是否变化
        if Some(&current_hash) == last_hash.as_ref() {
            // 无变化：查找最近的备份文件并重命名
            if let Some(latest_date) = state.storage.find_latest_backup(table_name).await? {
                if latest_date != date {
                    state.storage.rename(table_name, &latest_date, date).await?;
                    return Ok(TableBackupResult::Renamed);
                } else {
                    return Ok(TableBackupResult::Skipped("already up to date".to_string()));
                }
            } else {
                return Ok(TableBackupResult::Skipped("no existing backup found".to_string()));
            }
        }

        // 4. 有变化：导出新 CSV
        let (csv_path, _) = CsvExporter::export(&state.db, table_name, &state.temp_dir).await?;

        // 5. 保存到本地 + S3
        state.storage.save(table_name, date, &csv_path).await?;

        // 6. 更新哈希缓存
        state.last_hashes.write().await.insert(table_name.to_string(), current_hash);

        // 7. 清理临时文件
        let _ = std::fs::remove_file(csv_path);

        Ok(TableBackupResult::Exported)
    }
}

#[derive(Debug, Default)]
pub struct BackupResult {
    pub success: u32,
    pub failed: u32,
    pub exported: u32,
    pub renamed: u32,
    pub skipped: u32,
    pub cleaned: u32,
    pub duration: std::time::Duration,
}

enum TableBackupResult {
    Exported,
    Renamed,
    Skipped(String),
}
