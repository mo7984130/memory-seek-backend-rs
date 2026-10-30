use serde::{Deserialize, Serialize};
use types_core::{UserId, cursor::TimeIdCursor};
use validator::Validate;

/// 管理员可触发的后台任务类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "admin/"))]
pub enum AdminJobKind {
    FaceFull,
    FaceIncremental,
    PersonFullScan,
    FaceSecondaryCluster,
    BackupScheduled,
    BackupManual,
}

impl AdminJobKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FaceFull => "face_full",
            Self::FaceIncremental => "face_incremental",
            Self::PersonFullScan => "person_full_scan",
            Self::FaceSecondaryCluster => "face_secondary_cluster",
            Self::BackupScheduled => "backup_scheduled",
            Self::BackupManual => "backup_manual",
        }
    }
}

/// 后台任务状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "admin/"))]
pub enum AdminJobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    CancelRequested,
    Cancelled,
    Interrupted,
}

impl AdminJobStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::CancelRequested => "cancel_requested",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

fn default_job_page_size() -> u64 {
    32
}

/// 后台任务列表查询条件。
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "admin/"))]
pub struct AdminJobQuery {
    pub kind: Option<AdminJobKind>,
    pub status: Option<AdminJobStatus>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<String>>,
    #[serde(default = "default_job_page_size")]
    #[validate(range(min = 1, max = 100, message = "size 在 1 到 100 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: u64,
}

/// 启动后台任务的参数。
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "admin/"))]
pub struct StartAdminJobRequest {
    pub kind: AdminJobKind,
    /// 二次聚类阈值，仅 `face_secondary_cluster` 使用。
    #[validate(range(min = 0.0, max = 1.0, message = "threshold 在 0 到 1 之间"))]
    pub threshold: Option<f32>,
}

/// 后台任务信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "admin/"))]
pub struct AdminJobView {
    pub id: String,
    pub kind: AdminJobKind,
    pub status: AdminJobStatus,
    pub requested_by: UserId,
    pub created_at: common_core::time::DateTime,
    pub started_at: Option<common_core::time::DateTime>,
    pub finished_at: Option<common_core::time::DateTime>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}
