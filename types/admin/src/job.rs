use serde::{Deserialize, Serialize};
use types_backup::RestoreRequest;
use types_core::{UserId, cursor::TimeIdCursor};
use validator::{Validate, ValidationError, ValidationErrors};

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
    #[cfg_attr(feature = "orm", sea_orm(string_value = "backup_restore"))]
    BackupRestore,
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
            Self::BackupRestore => "backup_restore",
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
    /// 恢复参数，仅 `backup_restore` 使用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<RestoreRequest>,
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
///
/// 执行参数收拢在对应任务类型的变体内，从类型上排除「类型与参数不匹配」的非法状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StartAdminJobRequest {
    FaceFull,
    FaceIncremental,
    PersonFullScan,
    /// 二次聚类；`threshold` 为相似度阈值。
    FaceSecondaryCluster {
        threshold: f32,
    },
    BackupScheduled,
    BackupManual,
    /// 备份恢复；参数包含来源、层级与确认 ID。
    BackupRestore(RestoreRequest),
}

impl StartAdminJobRequest {
    /// 对应的持久化任务类型。
    pub const fn kind(&self) -> AdminJobKind {
        match self {
            Self::FaceFull => AdminJobKind::FaceFull,
            Self::FaceIncremental => AdminJobKind::FaceIncremental,
            Self::PersonFullScan => AdminJobKind::PersonFullScan,
            Self::FaceSecondaryCluster { .. } => AdminJobKind::FaceSecondaryCluster,
            Self::BackupScheduled => AdminJobKind::BackupScheduled,
            Self::BackupManual => AdminJobKind::BackupManual,
            Self::BackupRestore(_) => AdminJobKind::BackupRestore,
        }
    }

    /// 要持久化的任务执行参数。
    pub fn payload(&self) -> serde_json::Value {
        match self {
            Self::FaceSecondaryCluster { threshold } => {
                serde_json::json!({ "threshold": threshold })
            }
            Self::BackupRestore(request) => {
                serde_json::json!({ "restore": request })
            }
            _ => serde_json::Value::Object(Default::default()),
        }
    }
}

impl Validate for StartAdminJobRequest {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        match self {
            Self::FaceSecondaryCluster { threshold } => {
                if !(0.0..=1.0).contains(threshold) {
                    let mut error = ValidationError::new("range");
                    error.message = Some("threshold 在 0 到 1 之间".into());
                    errors.add("threshold", error);
                }
            }
            Self::BackupRestore(request) => {
                if request.run_id.is_empty() {
                    let mut error = ValidationError::new("required");
                    error.message = Some("run_id 不能为空".into());
                    errors.add("runId", error);
                }
                if request.run_id != request.confirm_run_id {
                    let mut error = ValidationError::new("mismatch");
                    error.message = Some("恢复确认 ID 不匹配".into());
                    errors.add("confirmRunId", error);
                }
            }
            _ => {}
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
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
    use types_backup::{BackupSource, BackupTier};

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
            ..Default::default()
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
    fn start_request_serializes_params_with_kind() {
        let request = StartAdminJobRequest::FaceSecondaryCluster { threshold: 0.7 };
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            r#"{"kind":"face_secondary_cluster","threshold":0.7}"#
        );
        assert_eq!(request.kind(), AdminJobKind::FaceSecondaryCluster);
        assert_eq!(
            request.payload(),
            serde_json::json!({ "threshold": f64::from(0.7_f32) })
        );

        let request: StartAdminJobRequest =
            serde_json::from_str(r#"{"kind":"face_secondary_cluster","threshold":0.7}"#).unwrap();
        assert_eq!(
            request,
            StartAdminJobRequest::FaceSecondaryCluster { threshold: 0.7 }
        );
    }

    #[test]
    fn start_request_omits_params_for_jobs_without_them() {
        let value = serde_json::to_value(StartAdminJobRequest::FaceFull).unwrap();
        assert_eq!(value, serde_json::json!({ "kind": "face_full" }));
    }

    #[test]
    fn start_request_validates_threshold_range() {
        assert!(
            StartAdminJobRequest::FaceSecondaryCluster { threshold: 0.7 }
                .validate()
                .is_ok()
        );
        assert!(
            StartAdminJobRequest::FaceSecondaryCluster { threshold: 1.5 }
                .validate()
                .is_err()
        );
        assert!(StartAdminJobRequest::BackupManual.validate().is_ok());
    }

    #[test]
    fn start_request_carries_restore_params() {
        let request = StartAdminJobRequest::BackupRestore(RestoreRequest {
            tier: BackupTier::Daily,
            source: BackupSource::Local,
            run_id: "run-1".to_owned(),
            confirm_run_id: "run-1".to_owned(),
        });
        assert_eq!(request.kind(), AdminJobKind::BackupRestore);
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            serde_json::json!({
                "kind": "backup_restore",
                "tier": "daily",
                "source": "local",
                "runId": "run-1",
                "confirmRunId": "run-1",
            })
        );
        assert_eq!(
            request.payload(),
            serde_json::json!({
                "restore": {
                    "tier": "daily",
                    "source": "local",
                    "runId": "run-1",
                    "confirmRunId": "run-1",
                }
            })
        );
    }

    #[test]
    fn start_request_rejects_mismatched_restore_confirmation() {
        let request = StartAdminJobRequest::BackupRestore(RestoreRequest {
            tier: BackupTier::Daily,
            source: BackupSource::Local,
            run_id: "run-1".to_owned(),
            confirm_run_id: "run-2".to_owned(),
        });
        assert!(request.validate().is_err());
    }

    #[test]
    fn payload_parses_threshold_without_restore() {
        let payload: AdminJobPayload =
            serde_json::from_value(serde_json::json!({ "threshold": 0.7 })).unwrap();
        assert_eq!(payload.threshold, Some(0.7));
        assert!(payload.restore.is_none());
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
