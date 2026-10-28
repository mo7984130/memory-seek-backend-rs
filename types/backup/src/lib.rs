//! 备份上下文契约:备份存储层级、恢复来源、请求与结果。

use serde::{Deserialize, Serialize};

/// 备份存储层级。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupTier {
    Daily,
    Weekly,
    Monthly,
    Manual,
}

impl BackupTier {
    /// 返回该备份层级对应的相对目录
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupSource {
    Local,
    S3,
}

/// 管理员触发备份恢复的请求。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    pub tier: BackupTier,
    pub source: BackupSource,
    pub run_id: String,
    pub confirm_run_id: String,
}

/// 定时备份(trigger)的执行结果。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledBackupResult {
    /// 成功导出的表数量
    pub exported: u32,
    /// 导出失败的表数量
    pub failed: u32,
    /// 清理掉的过期备份数量
    pub cleaned: u32,
    /// 本次执行耗时(秒)
    pub duration_secs: f64,
}

/// 管理员手动备份(manual)的执行结果。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualBackupResult {
    /// 成功导出的表数量
    pub exported: u32,
    /// 导出失败的表数量
    pub failed: u32,
    /// 本次执行耗时(秒)
    pub duration_secs: f64,
}

/// 备份恢复(restore)的执行结果。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    /// 恢复的数据行数
    pub restored: u64,
}
