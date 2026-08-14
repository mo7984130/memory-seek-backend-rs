use serde::Deserialize;

/// 定时备份调度配置（GFS 分层保留策略）
#[derive(Debug, Clone, Deserialize)]
pub struct BackupScheduleConfig {
    /// Cron 表达式，默认每天凌晨 6 点
    #[serde(default = "default_schedule_cron")]
    pub schedule: String,

    /// 日备份保留数量
    #[serde(default = "default_daily_retention")]
    pub daily_retention: u32,

    /// 周备份保留数量
    #[serde(default = "default_weekly_retention")]
    pub weekly_retention: u32,

    /// 月备份保留数量
    #[serde(default = "default_monthly_retention")]
    pub monthly_retention: u32,
}

impl Default for BackupScheduleConfig {
    fn default() -> Self {
        Self {
            schedule: default_schedule_cron(),
            daily_retention: default_daily_retention(),
            weekly_retention: default_weekly_retention(),
            monthly_retention: default_monthly_retention(),
        }
    }
}

/// 返回定时备份的默认 cron 表达式.
fn default_schedule_cron() -> String {
    "0 0 6 * * *".to_string()
}

/// 返回 daily 备份的默认保留数量.
fn default_daily_retention() -> u32 {
    7
}

/// 返回 weekly 备份的默认保留数量.
fn default_weekly_retention() -> u32 {
    4
}

/// 返回 monthly 备份的默认保留数量.
fn default_monthly_retention() -> u32 {
    6
}

/// 备份配置
#[derive(Debug, Clone, Deserialize)]
pub struct BackupConfig {
    /// 本地备份路径
    #[serde(default = "default_local_path")]
    pub local_path: String,

    /// S3 前缀
    #[serde(default = "default_s3_prefix")]
    pub s3_prefix: String,

    /// 备份的表名列表，None = 备份所有表
    #[serde(default)]
    pub tables: Option<Vec<String>>,

    /// 定时备份调度配置
    #[serde(default)]
    pub scheduled: BackupScheduleConfig,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            local_path: default_local_path(),
            s3_prefix: default_s3_prefix(),
            tables: None,
            scheduled: BackupScheduleConfig::default(),
        }
    }
}

/// 返回本地备份目录的默认路径.
fn default_local_path() -> String {
    "/var/backups/memory-seek".to_string()
}

/// 返回对象存储备份前缀的默认值.
fn default_s3_prefix() -> String {
    "backup/".to_string()
}

impl BackupConfig {
    /// 验证配置有效性
    pub fn validate(&self) -> Result<(), String> {
        if self.local_path.is_empty() {
            return Err("backup.local_path cannot be empty".to_string());
        }
        if self.scheduled.daily_retention == 0 {
            return Err("backup.scheduled.daily_retention must be > 0".to_string());
        }
        if self.scheduled.weekly_retention == 0 {
            return Err("backup.scheduled.weekly_retention must be > 0".to_string());
        }
        if self.scheduled.monthly_retention == 0 {
            return Err("backup.scheduled.monthly_retention must be > 0".to_string());
        }
        Ok(())
    }
}
