// ============================================================
// TimelineStatId
// ============================================================

crate::id_type!(TimelineStatId, String, "photo/");

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_timeline_stat")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub date_str: TimelineStatId,
        pub count: i64,
        pub anchor_time: DateTime,
        pub created_at: DateTime,
        pub updated_at: DateTime,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;
