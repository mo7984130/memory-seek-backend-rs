use std::fmt;

// ============================================================
// TimelineStatId
// ============================================================

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct TimelineStatId(pub String);

impl fmt::Display for TimelineStatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================
// SeaORM 支持（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
impl From<TimelineStatId> for sea_orm::Value {
    fn from(val: TimelineStatId) -> Self {
        sea_orm::Value::String(Some(Box::new(val.0)))
    }
}

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_timeline_stat")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub date_str: String,
        pub count: i64,
        pub anchor_time: DateTimeUtc,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;
