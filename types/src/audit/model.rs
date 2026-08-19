//! 审计查询接口的请求与响应 DTO。

use common::DateTime;

use crate::auth::user::UserId;
use crate::cursor::TimeIdCursor;
use crate::photo::behavior::{BehaviorTargetType, UserBehaviorAction, UserBehaviorId};
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
    pub action: Option<UserBehaviorAction>,
    pub target_type: Option<BehaviorTargetType>,
    pub start: Option<DateTime>,
    pub end: Option<DateTime>,
    pub granularity: AuditGranularity,
});

impl Default for AuditStatsQuery {
    fn default() -> Self {
        Self {
            action: None,
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
    pub action: UserBehaviorAction,
    pub target_type: BehaviorTargetType,
    #[validate(range(min = 1, max = 100, message = "limit 在 1 到 100 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub limit: u64,
});

impl Default for AuditTopQuery {
    fn default() -> Self {
        Self {
            action: UserBehaviorAction::View,
            target_type: BehaviorTargetType::Photo,
            limit: 3,
        }
    }
}

crate::in_dto!(AuditQuery, "audit/", serde_default, docs = "审计流水查询参数"; {
    pub action: Option<UserBehaviorAction>,
    pub target_type: Option<BehaviorTargetType>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub target_id: Option<i64>,
    pub user_id: Option<UserId>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<UserBehaviorId>>,
    #[validate(range(min = 1, max = 100, message = "size 在 1 到 100 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

impl Default for AuditQuery {
    fn default() -> Self {
        Self {
            action: None,
            target_type: None,
            target_id: None,
            user_id: None,
            cursor: None,
            size: 32,
        }
    }
}

crate::out_dto!(AuditItem, "audit/", docs = "审计流水响应项"; {
    pub id: UserBehaviorId,
    pub user_id: UserId,
    pub action: UserBehaviorAction,
    pub target_type: Option<BehaviorTargetType>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub target_id: Option<i64>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub detail: Option<serde_json::Value>,
    pub created_at: DateTime,
});
