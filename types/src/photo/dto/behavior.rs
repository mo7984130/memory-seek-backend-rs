//! 用户行为审计相关的管理端 DTO

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::auth::user::UserId;
use crate::cursor::TimeIdCursor;
use crate::photo::behavior::UserBehaviorId;
use crate::photo::behavior::{BehaviorTargetType, UserBehaviorAction};

/// 行为量聚合粒度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "photo/"))]
pub enum BehaviorGranularity {
    #[default]
    Day,
    Week,
    Month,
}

impl BehaviorGranularity {
    /// 对应 date_trunc 的时间桶单位
    pub const fn as_trunc(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }
}

crate::in_dto!(BehaviorStatsQuery, "photo/", serde_default, docs = "行为量聚合查询参数"; {
    pub action: Option<UserBehaviorAction>,
    pub target_type: Option<BehaviorTargetType>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub granularity: BehaviorGranularity,
});

impl Default for BehaviorStatsQuery {
    fn default() -> Self {
        Self {
            action: None,
            target_type: None,
            start: None,
            end: None,
            granularity: BehaviorGranularity::Day,
        }
    }
}

crate::out_dto!(BehaviorStatsItem, "photo/", docs = "行为量聚合结果项"; {
    pub bucket: DateTime<Utc>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: i64,
});

crate::out_dto!(BehaviorTopItem, "photo/", docs = "热门目标排行结果项"; {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub target_id: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: i64,
});

crate::in_dto!(BehaviorTopQuery, "photo/", serde_default, docs = "热门目标排行查询参数"; {
    pub action: UserBehaviorAction,
    pub target_type: BehaviorTargetType,
    #[validate(range(min = 1, max = 100, message = "limit 在 1 到 100 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub limit: u64,
});

impl Default for BehaviorTopQuery {
    fn default() -> Self {
        Self {
            action: UserBehaviorAction::View,
            target_type: BehaviorTargetType::Photo,
            limit: 3,
        }
    }
}

crate::in_dto!(BehaviorAuditQuery, "photo/", serde_default, docs = "审计流水查询参数"; {
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

impl Default for BehaviorAuditQuery {
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

crate::out_dto!(BehaviorAuditItem, "photo/", docs = "审计流水响应项"; {
    pub id: UserBehaviorId,
    pub user_id: UserId,
    pub action: UserBehaviorAction,
    pub target_type: Option<BehaviorTargetType>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub target_id: Option<i64>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub detail: Option<serde_json::Value>,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
});
