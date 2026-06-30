use oss::S3Client;
use std::path::PathBuf;
use std::sync::Arc;

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
        date: &str,
        csv_path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file_name = format!("{}.csv", date);

        // 1. 保存到本地
        let local_dir = self.local_path.join(table_name);
        std::fs::create_dir_all(&local_dir)?;
        let local_file = local_dir.join(&file_name);
        std::fs::copy(csv_path, &local_file)?;

        // 2. 上传到 S3
        let s3_key = format!("{}{}/{}", self.s3_prefix, table_name, file_name);
        let csv_content = std::fs::read(csv_path)?;
        self.s3_client
            .upload(&s3_key, csv_content, "text/csv")
            .await?;

        tracing::info!(
            table = %table_name,
            date = %date,
            local = %local_file.display(),
            s3 = %s3_key,
            "Backup saved"
        );

        Ok(())
    }

    /// 查找指定表的最新备份日期
    pub async fn find_latest_backup(
        &self,
        table_name: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let local_dir = self.local_path.join(table_name);
        if !local_dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<String> = None;

        for entry in std::fs::read_dir(&local_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            if file_name.ends_with(".csv") {
                let date = file_name.trim_end_matches(".csv").to_string();
                if latest.as_ref().map_or(true, |l| &date > l) {
                    latest = Some(date);
                }
            }
        }

        Ok(latest)
    }

    /// 重命名文件（当数据无变化时）
    pub async fn rename(
        &self,
        table_name: &str,
        old_date: &str,
        new_date: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let old_file = format!("{}.csv", old_date);
        let new_file = format!("{}.csv", new_date);

        // 1. 重命名本地文件
        let local_dir = self.local_path.join(table_name);
        let old_local = local_dir.join(&old_file);
        let new_local = local_dir.join(&new_file);

        if old_local.exists() {
            std::fs::rename(&old_local, &new_local)?;
            tracing::info!(
                table = %table_name,
                from = %old_date,
                to = %new_date,
                "Local backup renamed"
            );
        }

        // 2. 重命名 S3 文件（复制+删除）
        let old_s3_key = format!("{}{}/{}", self.s3_prefix, table_name, old_file);
        let new_s3_key = format!("{}{}/{}", self.s3_prefix, table_name, new_file);

        // 下载旧文件
        match self.s3_client.download(&old_s3_key).await {
            Ok(content) => {
                // 上传为新文件
                self.s3_client
                    .upload(&new_s3_key, content.to_vec(), "text/csv")
                    .await?;
                // 删除旧文件
                self.s3_client.delete(&old_s3_key).await?;
                tracing::info!(
                    table = %table_name,
                    from = %old_s3_key,
                    to = %new_s3_key,
                    "S3 backup renamed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    table = %table_name,
                    key = %old_s3_key,
                    err = ?e,
                    "S3 old file not found, skip rename"
                );
            }
        }

        Ok(())
    }

    /// 清理过期备份
    pub async fn cleanup(
        &self,
        retention_days: u32,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let cutoff = chrono::Local::now() - chrono::Duration::days(retention_days as i64);
        let cutoff_str = cutoff.format("%Y-%m-%d").to_string();

        let mut removed_count = 0;

        // 遍历本地备份目录
        if self.local_path.exists() {
            for entry in std::fs::read_dir(&self.local_path)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }

                let table_name = entry.file_name().to_string_lossy().to_string();
                let table_dir = entry.path();

                for file_entry in std::fs::read_dir(&table_dir)? {
                    let file_entry = file_entry?;
                    let file_name = file_entry.file_name();
                    let file_name = file_name.to_string_lossy();

                    if file_name.ends_with(".csv") {
                        let date = file_name.trim_end_matches(".csv");
                        if date < cutoff_str.as_str() {
                            // 删除本地文件
                            std::fs::remove_file(file_entry.path())?;

                            // 删除 S3 文件
                            let s3_key =
                                format!("{}{}/{}", self.s3_prefix, table_name, file_name);
                            let _ = self.s3_client.delete(&s3_key).await;

                            removed_count += 1;
                            tracing::info!(
                                table = %table_name,
                                date = %date,
                                "Removed expired backup"
                            );
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }
}
