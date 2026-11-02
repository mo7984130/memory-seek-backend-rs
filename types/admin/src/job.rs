use serde::{Deserialize, Serialize};
use types_core::{UserId, cursor::TimeIdCursor};
use validator::Validate;

#[cfg(feature = "orm")]
use sea_orm::sea_query::StringLen;

/// 管理员可触发的后台任务类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "orm", derive(sea_orm::EnumIter, sea_orm::DeriveActiveEnum))]
#[cfg_attr(
    feature = "orm",
    sea_orm(rs_type = "String", db_type = "String(StringLen::N(64))")
)]
#[serde(rename_all = "snake_case")]
pub enum AdminJobKind {
    #[cfg_attr(feature = "orm", sea_orm(string_value = "face_full"))]
    FaceFull,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "face_incremental"))]
    FaceIncremental,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "person_full_scan"))]
    PersonFullScan,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "face_secondary_cluster"))]
    FaceSecondaryCluster,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "backup_scheduled"))]
    BackupScheduled,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "backup_manual"))]
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
///
/// 多实例下状态由数据库唯一事实源维护：实例崩溃留下的 `running` 任务会在
/// 心跳中断后落到 `failed`，需要管理员人工重新触发。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "orm", derive(sea_orm::EnumIter, sea_orm::DeriveActiveEnum))]
#[cfg_attr(
    feature = "orm",
    sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")
)]
#[serde(rename_all = "snake_case")]
pub enum AdminJobStatus {
    #[cfg_attr(feature = "orm", sea_orm(string_value = "queued"))]
    Queued,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "running"))]
    Running,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "succeeded"))]
    Succeeded,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "failed"))]
    Failed,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "cancel_requested"))]
    CancelRequested,
    #[cfg_attr(feature = "orm", sea_orm(string_value = "cancelled"))]
    Cancelled,
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
        }
    }

    /// 是否处于占用「同类型唯一活动任务」约束的状态。
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::CancelRequested)
    }
}

/// 持久化的任务执行参数。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminJobPayload {
    /// 二次聚类阈值，仅 `face_secondary_cluster` 使用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
}

fn default_job_page_size() -> u64 {
    32
}

/// 后台任务列表查询条件。
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AdminJobQuery {
    pub kind: Option<AdminJobKind>,
    pub status: Option<AdminJobStatus>,
    pub cursor: Option<TimeIdCursor<String>>,
    #[serde(default = "default_job_page_size")]
    #[validate(range(min = 1, max = 100, message = "size 在 1 到 100 之间"))]
    pub size: u64,
}

/// 人工放弃运行中任务的参数。
#[derive(Debug, Clone, Default, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AbandonAdminJobRequest {
    /// 放弃原因，写入任务的 `error` 字段。
    #[validate(length(max = 512, message = "原因不能超过512个字符"))]
    pub reason: Option<String>,
}

/// 启动后台任务的参数。
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StartAdminJobRequest {
    pub kind: AdminJobKind,
    /// 二次聚类阈值，仅 `face_secondary_cluster` 使用。
    #[validate(range(min = 0.0, max = 1.0, message = "threshold 在 0 到 1 之间"))]
    pub threshold: Option<f32>,
}

/// 后台任务信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminJobView {
    pub id: String,
    pub kind: AdminJobKind,
    pub status: AdminJobStatus,
    pub requested_by: UserId,
    /// 已开始执行的次数（含当前这次）。
    pub attempts: u32,
    /// 当前持有任务的实例标识；任务卡死时可用于定位实例。
    pub worker_id: Option<String>,
    pub created_at: common_core::time::DateTime,
    pub started_at: Option<common_core::time::DateTime>,
    pub finished_at: Option<common_core::time::DateTime>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_names_are_stable() {
        assert_eq!(AdminJobKind::FaceFull.as_str(), "face_full");
        assert_eq!(AdminJobKind::BackupManual.as_str(), "backup_manual");
        assert_eq!(AdminJobStatus::CancelRequested.as_str(), "cancel_requested");
    }

    #[test]
    fn payload_round_trips_threshold() {
        let payload = AdminJobPayload {
            threshold: Some(0.7),
        };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            serde_json::from_value::<AdminJobPayload>(value).unwrap(),
            payload
        );
    }

    #[test]
    fn empty_payload_omits_threshold() {
        let value = serde_json::to_value(AdminJobPayload::default()).unwrap();
        assert_eq!(value, serde_json::json!({}));
    }

    #[test]
    fn active_statuses_are_the_unique_set() {
        assert!(AdminJobStatus::Queued.is_active());
        assert!(AdminJobStatus::Running.is_active());
        assert!(AdminJobStatus::CancelRequested.is_active());
        assert!(!AdminJobStatus::Succeeded.is_active());
        assert!(!AdminJobStatus::Failed.is_active());
        assert!(!AdminJobStatus::Cancelled.is_active());
    }
}
