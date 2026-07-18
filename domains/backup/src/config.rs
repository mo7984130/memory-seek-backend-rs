use serde::Deserialize;

/// 备份配置
#[derive(Debug, Clone, Deserialize)]
pub struct BackupConfig {
    /// 是否启用备份
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Cron 表达式，默认每天凌晨 6 点
    #[serde(default = "default_schedule")]
    pub schedule: String,

    /// 本地备份路径
    #[serde(default = "default_local_path")]
    pub local_path: String,

    /// S3 前缀
    #[serde(default = "default_s3_prefix")]
    pub s3_prefix: String,

    /// 保留天数
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    /// 备份的表名列表，None = 备份所有表
    #[serde(default)]
    pub tables: Option<Vec<String>>,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            schedule: default_schedule(),
            local_path: default_local_path(),
            s3_prefix: default_s3_prefix(),
            retention_days: default_retention_days(),
            tables: None,
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_schedule() -> String {
    "0 0 6 * * *".to_string()
}

fn default_local_path() -> String {
    "/var/backups/memory-seek".to_string()
}

fn default_s3_prefix() -> String {
    "backup/".to_string()
}

fn default_retention_days() -> u32 {
    3
}

impl BackupConfig {
    /// 验证配置有效性
    pub fn validate(&self) -> Result<(), String> {
        if self.local_path.is_empty() {
            return Err("backup.local_path cannot be empty".to_string());
        }
        if self.retention_days == 0 {
            return Err("backup.retention_days must be > 0".to_string());
        }
        Ok(())
    }
}
