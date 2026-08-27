//! 审计查询接口的请求与响应 DTO。

use common::time::DateTime;

use super::AuditEventId;
use crate::audit::AuditRecord;
use crate::auth::user::UserId;
use crate::cursor::TimeIdCursor;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "audit/"))]
pub enum AuditGranularity {
    #[default]
    Day,
    Week,
    Month,
}

impl AuditGranularity {
    pub const fn as_trunc(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }
}

crate::in_dto!(AuditStatsQuery, "audit/", serde_default, docs = "审计统计查询参数"; {
    pub event_type: Option<String>,
    pub target_type: Option<String>,
    pub start: Option<DateTime>,
    pub end: Option<DateTime>,
    pub granularity: AuditGranularity,
});

impl Default for AuditStatsQuery {
    fn default() -> Self {
        Self {
            event_type: None,
            target_type: None,
            start: None,
            end: None,
            granularity: AuditGranularity::Day,
        }
    }
}

crate::out_dto!(AuditStatsItem, "audit/", docs = "审计统计结果项"; {
    pub bucket: DateTime,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: i64,
});

crate::out_dto!(AuditTopItem, "audit/", docs = "审计热门目标结果项"; {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub target_id: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: i64,
});

crate::in_dto!(AuditTopQuery, "audit/", serde_default, docs = "审计热门目标查询参数"; {
    pub event_type: String,
    pub target_type: String,
    #[validate(range(min = 1, max = 100, message = "limit 在 1 到 100 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub limit: u64,
});

impl Default for AuditTopQuery {
    fn default() -> Self {
        Self {
            event_type: "view".to_owned(),
            target_type: "photo".to_owned(),
            limit: 3,
        }
    }
}

crate::in_dto!(AuditQuery, "audit/", serde_default, docs = "审计流水查询参数"; {
    pub event_type: Option<String>,
    pub target_type: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub target_id: Option<i64>,
    pub actor_id: Option<UserId>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<AuditEventId>>,
    #[validate(range(min = 1, max = 100, message = "size 在 1 到 100 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

impl Default for AuditQuery {
    fn default() -> Self {
        Self {
            event_type: None,
            target_type: None,
            target_id: None,
            actor_id: None,
            cursor: None,
            size: 32,
        }
    }
}

crate::out_dto!(AuditItem, "audit/", docs = "审计流水响应项"; {
    pub id: AuditEventId,
    pub event_type: String,
    pub actor_id: Option<UserId>,
    pub target_type: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub target_id: Option<i64>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub detail: Option<serde_json::Value>,
    pub created_at: DateTime,
});
impl From<AuditRecord> for AuditItem {
    fn from(record: AuditRecord) -> Self {
        Self {
            id: record.id,
            event_type: record.event_type,
            actor_id: record.actor_id.map(UserId),
            target_type: record.target_type,
            target_id: record.target_id,
            detail: record.detail,
            created_at: record.created_at,
        }
    }
}
