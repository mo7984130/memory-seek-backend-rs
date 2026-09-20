use crate::config::BackupScheduleConfig;
use crate::error::BackupError;
use crate::manifest::{BackupManifest, FILE_NAME};
use oss::S3Client;
use std::path::PathBuf;

pub use types::backup::{BackupSource, BackupTier};

/// 备份存储管理器
#[derive(Clone)]
pub struct BackupStorage {
    local_path: PathBuf,
    s3_client: S3Client,
    s3_prefix: String,
}

impl BackupStorage {
    /// 创建本地与对象存储备份的统一存储入口.
    pub fn new(local_path: PathBuf, s3_client: S3Client, s3_prefix: String) -> Self {
        Self {
            local_path,
            s3_client,
            s3_prefix,
        }
    }

    /// 保存已导出的归档文件到本地和 S3。
    pub async fn save(
        &self,
        table_name: &str,
        archive_path: &std::path::Path,
        tier: BackupTier,
        run_id: &str,
    ) -> Result<(), BackupError> {
        let relative_file = Self::relative_file_path(tier, run_id, table_name);
        let local_file = self.local_path.join(&relative_file);
        let parent = local_file.parent().expect("备份文件路径必须包含父目录");
        std::fs::create_dir_all(parent)?;
        std::fs::copy(archive_path, &local_file)?;

        let s3_key = self.object_key(&relative_file);
        self.s3_client
            .upload_file(&s3_key, archive_path, "application/zstd")
            .await?;

        Ok(())
    }

    pub async fn save_manifest(
        &self,
        tier: BackupTier,
        run_id: &str,
        manifest: &BackupManifest,
    ) -> Result<(), BackupError> {
        let relative = Self::relative_manifest_path(tier, run_id);
        let bytes = serde_json::to_vec(manifest)?;
        let local_file = self.local_path.join(&relative);
        let parent = local_file.parent().expect("备份清单路径必须包含父目录");
        tokio::fs::create_dir_all(parent).await?;
        tokio::fs::write(&local_file, &bytes).await?;
        self.s3_client
            .upload(&self.object_key(&relative), &bytes, "application/json")
            .await?;
        Ok(())
    }

    pub async fn load_local_manifest(
        &self,
        tier: BackupTier,
        run_id: &str,
    ) -> Result<BackupManifest, BackupError> {
        let bytes = tokio::fs::read(
            self.local_path
                .join(Self::relative_manifest_path(tier, run_id)),
        )
        .await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn load_manifest(
        &self,
        source: BackupSource,
        tier: BackupTier,
        run_id: &str,
    ) -> Result<BackupManifest, BackupError> {
        let relative = Self::relative_manifest_path(tier, run_id);
        let bytes = match source {
            BackupSource::Local => tokio::fs::read(self.local_path.join(relative)).await?,
            BackupSource::S3 => self
                .s3_client
                .download(&self.object_key(&relative))
                .await?
                .to_vec(),
        };
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn s3_archive_stream(
        &self,
        tier: BackupTier,
        run_id: &str,
        table: &str,
    ) -> Result<
        impl futures_util::Stream<Item = Result<bytes::Bytes, oss::OssError>> + use<>,
        BackupError,
    > {
        self.s3_client
            .get_download_stream_response(
                &self.object_key(&Self::relative_file_path(tier, run_id, table)),
            )
            .await
            .map_err(BackupError::from)
    }

    pub fn local_archive_path(&self, tier: BackupTier, run_id: &str, table: &str) -> PathBuf {
        self.local_path
            .join(Self::relative_file_path(tier, run_id, table))
    }

    /// GFS 分层清理：本地与 S3 各自按保留数清理 daily / weekly / monthly 目录。
    ///
    /// 本地与 S3 独立枚举、独立截断：即使本地目录丢失（迁移/清理）或存在仅
    /// S3 上可见的历史 run（如多实例部署、备份收尾失败），S3 端也能收敛到保留数内。
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
        removed += self
            .cleanup_s3_subdir("scheduled/daily", config.daily_retention)
            .await?;
        removed += self
            .cleanup_s3_subdir("scheduled/weekly", config.weekly_retention)
            .await?;
        removed += self
            .cleanup_s3_subdir("scheduled/monthly", config.monthly_retention)
            .await?;

        Ok(removed)
    }

    /// 清理本地指定子目录下超出保留数的历史备份 run。
    ///
    /// 每个子目录是一个备份运行（按 run_id 命名），删除整个目录 = 删除该次所有表。
    /// 按 run_id 倒序保留最新，删除失败仅记录日志，不中断其余 run 的清理。
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

            if let Err(error) = std::fs::remove_dir_all(&run_dir) {
                tracing::error!(run = %run_id, dir = %rel_dir, error = %error, "本地 GFS 清理失败，跳过该备份 run");
                continue;
            }
            removed += 1;
            tracing::info!(run = %run_id, dir = %rel_dir, "GFS cleanup removed expired local backup run");
        }

        Ok(removed)
    }

    /// 清理 S3 指定前缀下超出保留数的历史备份 run。
    ///
    /// 直接从对象存储枚举 run，不依赖本地目录状态，避免本地丢失或多实例部署时
    /// S3 对象永久堆积。错误向上返回：清理是定时任务的一部分，失败即任务失败。
    async fn cleanup_s3_subdir(&self, rel_dir: &str, keep_count: u32) -> Result<u32, BackupError> {
        let prefix = Self::object_prefix(&self.s3_prefix, rel_dir);
        let keys = self.s3_client.list(&prefix).await?;

        // 按 run_id 分组（对象键形如 {prefix}/{run_id}/{file}）
        let runs = Self::group_keys_by_run(&prefix, &keys);

        // run_id 为 %Y%m%d_%H%M%S，字典序即时间序；倒序保留最新 keep_count 个
        let mut removed = 0;
        for (run_id, keys) in runs.iter().rev().skip(keep_count as usize) {
            self.s3_client.delete_batch(keys.to_vec()).await?;
            removed += 1;
            tracing::info!(run = %run_id, dir = %rel_dir, "S3 GFS cleanup removed expired backup run");
        }

        Ok(removed)
    }

    /// 构造 S3 上某相对目录的对象前缀（以 "/" 结尾，可直接用作 list 前缀）。
    fn object_prefix(s3_prefix: &str, rel_dir: &str) -> String {
        let suffix = format!("{rel_dir}/");
        if s3_prefix.is_empty() {
            suffix
        } else {
            format!("{s3_prefix}/{suffix}")
        }
    }

    /// 将 S3 对象 key 按 run_id 分组（对象键形如 {prefix}/{run_id}/{file}）。
    ///
    /// 无法解析出 run_id 的 key（孤儿对象）将被忽略，避免误删非备份对象。
    fn group_keys_by_run<'a>(
        prefix: &str,
        keys: &'a [String],
    ) -> std::collections::BTreeMap<String, Vec<&'a str>> {
        let mut runs: std::collections::BTreeMap<String, Vec<&'a str>> =
            std::collections::BTreeMap::new();
        for key in keys {
            if let Some(run_id) = key
                .strip_prefix(prefix)
                .and_then(|rest| rest.split('/').next())
                .filter(|run_id| !run_id.is_empty())
            {
                runs.entry(run_id.to_string())
                    .or_default()
                    .push(key.as_str());
            }
        }
        runs
    }

    fn relative_file_path(tier: BackupTier, run_id: &str, table_name: &str) -> PathBuf {
        PathBuf::from(tier.rel_dir())
            .join(run_id)
            .join(format!("{table_name}.copy.zst"))
    }

    fn relative_manifest_path(tier: BackupTier, run_id: &str) -> PathBuf {
        PathBuf::from(tier.rel_dir()).join(run_id).join(FILE_NAME)
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

#[cfg(test)]
mod tests {
    use super::{BackupStorage, BackupTier};
    use std::path::PathBuf;

    #[test]
    fn backup_file_is_stored_directly_under_its_run() {
        assert_eq!(
            BackupStorage::relative_file_path(BackupTier::Daily, "20260828_120000", "face"),
            PathBuf::from("scheduled/daily/20260828_120000/face.copy.zst")
        );
    }

    #[test]
    fn object_prefix_appends_trailing_slash() {
        assert_eq!(
            BackupStorage::object_prefix("backup", "scheduled/daily"),
            "backup/scheduled/daily/"
        );
        assert_eq!(
            BackupStorage::object_prefix("", "scheduled/daily"),
            "scheduled/daily/"
        );
    }

    #[test]
    fn keys_are_grouped_by_run_id_in_time_order() {
        let keys = vec![
            "backup/scheduled/daily/20260901_060000/face.copy.zst".to_string(),
            "backup/scheduled/daily/20260901_060000/manifest.json".to_string(),
            "backup/scheduled/daily/20260902_060000/face.copy.zst".to_string(),
            "backup/scheduled/disabled/20260903_060000/face.copy.zst".to_string(),
        ];
        let runs = BackupStorage::group_keys_by_run("backup/scheduled/daily/", &keys);
        let ids: Vec<_> = runs.keys().collect();
        assert_eq!(ids, vec!["20260901_060000", "20260902_060000"]);
        assert_eq!(runs["20260901_060000"].len(), 2);
        assert_eq!(runs["20260902_060000"].len(), 1);
    }

    #[test]
    fn keys_outside_prefix_are_ignored() {
        let keys = vec![
            "unrelated/key.txt".to_string(),
            "backup/scheduled/daily/20260901_060000/manifest.json".to_string(),
        ];
        let runs = BackupStorage::group_keys_by_run("backup/scheduled/daily/", &keys);
        assert_eq!(runs.len(), 1);
        assert!(!runs.contains_key("unrelated"));
        assert!(runs.contains_key("20260901_060000"));
    }
}
