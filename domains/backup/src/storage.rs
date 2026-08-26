use crate::config::BackupScheduleConfig;
use crate::error::BackupError;
use oss::S3Client;
use std::path::PathBuf;
use std::sync::Arc;

/// 备份存储层级
#[derive(Clone, Copy)]
pub enum BackupTier {
    Daily,
    Weekly,
    Monthly,
    Manual,
}

impl BackupTier {
    fn rel_dir(&self) -> &'static str {
        match self {
            Self::Daily => "scheduled/daily",
            Self::Weekly => "scheduled/weekly",
            Self::Monthly => "scheduled/monthly",
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
    /// 创建本地与对象存储备份的统一存储入口.
    pub fn new(local_path: PathBuf, s3_client: Arc<S3Client>, s3_prefix: String) -> Self {
        Self {
            local_path,
            s3_client,
            s3_prefix,
        }
    }

    /// 保存已导出的文件到本地和 S3。
    pub async fn save(
        &self,
        table_name: &str,
        csv_path: &std::path::Path,
        tier: BackupTier,
        run_id: &str,
    ) -> Result<(), BackupError> {
        let relative_file = Self::relative_file_path(tier, run_id, table_name);
        let local_file = self.local_path.join(&relative_file);
        let parent = local_file.parent().expect("备份文件路径必须包含父目录");
        std::fs::create_dir_all(parent)?;
        std::fs::copy(csv_path, &local_file)?;

        let s3_key = self.object_key(&relative_file);
        self.s3_client
            .upload_file(&s3_key, csv_path, "text/csv")
            .await?;

        Ok(())
    }

    /// GFS 分层清理：按保留数清理 daily / weekly / monthly 目录
    pub async fn cleanup_gfs(&self, config: &BackupScheduleConfig) -> Result<u32, BackupError> {
        let mut removed = 0;
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
    async fn cleanup_subdir(&self, rel_dir: &str, keep_count: u32) -> Result<u32, BackupError> {
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

            if !s3_keys.is_empty() {
                self.s3_client.delete_batch(s3_keys).await?;
            }
            std::fs::remove_dir_all(&run_dir)?;

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

    /// 递归收集目录下的 CSV 文件对应的对象存储路径.
    fn collect_csv_keys(&self, dir: &std::path::Path, keys: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.collect_csv_keys(&path, keys);
                } else if path.extension().is_some_and(|e| e == "csv")
                    && let Ok(relative) = path.strip_prefix(&self.local_path)
                {
                    keys.push(self.object_key(relative));
                }
            }
        }
    }

    fn relative_file_path(tier: BackupTier, run_id: &str, table_name: &str) -> PathBuf {
        PathBuf::from(tier.rel_dir())
            .join(run_id)
            .join(table_name)
            .join(format!("{table_name}.csv"))
    }

    fn object_key(&self, relative_path: &std::path::Path) -> String {
        let path = relative_path.to_string_lossy();
        if self.s3_prefix.is_empty() {
            path.into_owned()
        } else {
            format!("{}/{path}", self.s3_prefix)
        }
    }
}
