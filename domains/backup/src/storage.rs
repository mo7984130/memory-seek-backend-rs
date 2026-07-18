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
    pub async fn save(
        &self,
        table_name: &str,
        key: &str,
        csv_path: &std::path::Path,
        backup_type: BackupType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file_name = format!("{}.csv", key);

        // 1. 保存到本地
        let local_dir = self.local_path.join(backup_type.rel_dir()).join(table_name);
        std::fs::create_dir_all(&local_dir)?;
        let local_file = local_dir.join(&file_name);
        std::fs::copy(csv_path, &local_file)?;

        // 2. 上传到 S3
        let s3_key = format!(
            "{}{}/{}/{}",
            self.s3_prefix,
            backup_type.rel_dir(),
            table_name,
            file_name
        );
        let csv_content = std::fs::read(csv_path)?;
        self.s3_client
            .upload(&s3_key, csv_content, "text/csv")
            .await?;

        tracing::info!(
            table = %table_name,
            key = %key,
            backup_type = %backup_type.rel_dir(),
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
        daily_key: &str,
        weekly_key: &str,
        monthly_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.save(table_name, daily_key, csv_path, BackupType::ScheduledDaily)
            .await?;
        self.save(table_name, weekly_key, csv_path, BackupType::ScheduledWeekly)
            .await?;
        self.save(table_name, monthly_key, csv_path, BackupType::ScheduledMonthly)
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

    /// 清理指定子目录下每个表目录中超过保留数的文件
    async fn cleanup_subdir(
        &self,
        rel_dir: &str,
        keep_count: u32,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let dir = self.local_path.join(rel_dir);
        if !dir.exists() {
            return Ok(0);
        }

        let mut removed = 0;

        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let table_name = entry.file_name().to_string_lossy().to_string();
            let table_dir = entry.path();

            let mut files: Vec<_> = std::fs::read_dir(&table_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .ends_with(".csv")
                })
                .collect();

            // 按文件名降序（最新的在前）
            files.sort_by(|a, b| {
                let a_name = a.file_name().to_string_lossy().to_string();
                let b_name = b.file_name().to_string_lossy().to_string();
                b_name.cmp(&a_name)
            });

            for file in files.iter().skip(keep_count as usize) {
                let file_name = file.file_name().to_string_lossy().to_string();

                // 删除本地文件
                std::fs::remove_file(file.path())?;

                // 删除 S3 文件
                let s3_key = format!("{}{}/{}/{}", self.s3_prefix, rel_dir, table_name, file_name);
                let _ = self.s3_client.delete(&s3_key).await;

                removed += 1;
                tracing::info!(
                    table = %table_name,
                    file = %file_name,
                    dir = %rel_dir,
                    "GFS cleanup removed expired backup"
                );
            }
        }

        Ok(removed)
    }
}
