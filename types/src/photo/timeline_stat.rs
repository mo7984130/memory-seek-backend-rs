// ============================================================
// TimelineStatId
// ============================================================
crate::id_type!(TimelineStatId, String, "photo/");
pub fn to_date_str(date: &common::time::DateTime) -> String {
    date.format("%Y-%m").to_string()
}

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod orm {
    use common::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_timeline_stat")]
    pub struct Model {
        /// 主键ID
        /// 也是日期(YYYY-MM)
        #[sea_orm(primary_key, auto_increment = false)]
        pub date_str: TimelineStatId,

        /// 照片数量
        pub count: u64,

        /// 本月份内, 最新一张照片的时间
        pub anchor_time: DateTime,

        /// 修改时间
        pub updated_at: DateTime,

        /// 创建时间
        pub created_at: DateTime,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use orm::*;
