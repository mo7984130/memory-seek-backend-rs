use crate::config::BackupScheduleConfig;
use oss::S3Client;
use std::path::PathBuf;
use std::sync::Arc;

/// 备份类型
pub enum BackupType {
    ScheduledDaily,
    ScheduledWeekly,
    ScheduledMonthly,
    Manual,
}

impl BackupType {
    fn rel_dir(&self) -> &'static str {
        match self {
            Self::ScheduledDaily => "scheduled/daily",
            Self::ScheduledWeekly => "scheduled/weekly",
            Self::ScheduledMonthly => "scheduled/monthly",
            Self::Manual => "manual",
        }
    }
}

/// 备份存储管理器
#[derive(Clone)]
pub struct BackupStorage {
    local_path: PathBuf,
    s3_client: Arc<S3Client>,
    s3_prefix: String,
}

impl BackupStorage {
    pub fn new(local_path: PathBuf, s3_client: Arc<S3Client>, s3_prefix: String) -> Self {
        Self {
            local_path,
            s3_client,
            s3_prefix,
        }
    }

    /// 保存备份文件到本地 + S3
    ///
    /// `run_id` 是该次备份运行的唯一标识（如 "20260719_060000"），会作为目录层级插入。
    pub async fn save(
        &self,
        table_name: &str,
        csv_path: &std::path::Path,
        backup_type: BackupType,
        run_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file_name = format!("{}.csv", table_name);

        let local_dir = self
            .local_path
            .join(backup_type.rel_dir())
            .join(run_id)
            .join(table_name);
        std::fs::create_dir_all(&local_dir)?;
        let local_file = local_dir.join(&file_name);
        std::fs::copy(csv_path, &local_file)?;

        let s3_key = format!(
            "{}{}/{}/{}/{}",
            self.s3_prefix,
            backup_type.rel_dir(),
            run_id,
            table_name,
            file_name
        );
        let csv_content = std::fs::read(csv_path)?;
        self.s3_client
            .upload(&s3_key, csv_content, "text/csv")
            .await?;

        tracing::info!(
            table = %table_name,
            backup_type = %backup_type.rel_dir(),
            run_id = %run_id,
            local = %local_file.display(),
            s3 = %s3_key,
            "Backup saved"
        );

        Ok(())
    }

    /// 一次性保存到 daily / weekly / monthly 三个目录
    pub async fn save_scheduled_all(
        &self,
        table_name: &str,
        csv_path: &std::path::Path,
        run_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.save(table_name, csv_path, BackupType::ScheduledDaily, run_id)
            .await?;
        self.save(table_name, csv_path, BackupType::ScheduledWeekly, run_id)
            .await?;
        self.save(table_name, csv_path, BackupType::ScheduledMonthly, run_id)
            .await?;
        Ok(())
    }

    /// GFS 分层清理：按保留数清理 daily / weekly / monthly 目录
    pub async fn cleanup_gfs(
        &self,
        config: &BackupScheduleConfig,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let mut removed = 0;

        if !self.local_path.exists() {
            return Ok(0);
        }

        removed += self
            .cleanup_subdir("scheduled/daily", config.daily_retention)
            .await?;
        removed += self
            .cleanup_subdir("scheduled/weekly", config.weekly_retention)
            .await?;
        removed += self
            .cleanup_subdir("scheduled/monthly", config.monthly_retention)
            .await?;
        // manual 目录不做清理

        Ok(removed)
    }

    /// 清理指定子目录下超出保留数的历史备份 run
    ///
    /// 每个子目录是一个备份运行（按 run_id 命名），删除整个目录 = 删除该次所有表。
    async fn cleanup_subdir(
        &self,
        rel_dir: &str,
        keep_count: u32,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let dir = self.local_path.join(rel_dir);
        if !dir.exists() {
            return Ok(0);
        }

        let mut run_dirs: Vec<_> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .collect();

        run_dirs.sort_by(|a, b| {
            let a_name = a.file_name().to_string_lossy().to_string();
            let b_name = b.file_name().to_string_lossy().to_string();
            b_name.cmp(&a_name)
        });

        let mut removed = 0;

        for entry in run_dirs.iter().skip(keep_count as usize) {
            let run_id = entry.file_name().to_string_lossy().to_string();
            let run_dir = entry.path();

            let s3_keys = self.collect_s3_keys_for_run(&run_dir);

            if let Err(e) = std::fs::remove_dir_all(&run_dir) {
                tracing::error!(run = %run_id, dir = %rel_dir, err = %e, "Failed to remove local backup dir");
                continue;
            }

            if !s3_keys.is_empty() {
                if let Err(e) = self.s3_client.delete_batch(s3_keys).await {
                    tracing::warn!(run = %run_id, dir = %rel_dir, err = %e, "GFS cleanup partial S3 deletion");
                }
            }

            removed += 1;
            tracing::info!(run = %run_id, dir = %rel_dir, "GFS cleanup removed expired backup run");
        }

        Ok(removed)
    }

    /// 收集一个 run 目录下所有 CSV 文件对应的 S3 路径
    fn collect_s3_keys_for_run(&self, run_dir: &std::path::Path) -> Vec<String> {
        let mut keys = Vec::new();
        self.collect_csv_keys(run_dir, &mut keys);
        keys
    }

    fn collect_csv_keys(&self, dir: &std::path::Path, keys: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.collect_csv_keys(&path, keys);
                } else if path.extension().is_some_and(|e| e == "csv") {
                    if let Ok(relative) = path.strip_prefix(&self.local_path) {
                        keys.push(format!("{}{}", self.s3_prefix, relative.display()));
                    }
                }
            }
        }
    }
}
