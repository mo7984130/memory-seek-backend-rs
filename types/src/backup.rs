use serde::Deserialize;

/// 备份存储层级。
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupTier {
    Daily,
    Weekly,
    Monthly,
    Manual,
}

impl BackupTier {
    pub const fn rel_dir(self) -> &'static str {
        match self {
            Self::Daily => "scheduled/daily",
            Self::Weekly => "scheduled/weekly",
            Self::Monthly => "scheduled/monthly",
            Self::Manual => "manual",
        }
    }
}

/// 恢复归档的存储来源。
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupSource {
    Local,
    S3,
}

/// 管理员触发备份恢复的请求。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    pub tier: BackupTier,
    pub source: BackupSource,
    pub run_id: String,
    pub confirm_run_id: String,
}
