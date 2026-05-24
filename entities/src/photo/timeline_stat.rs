use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub struct TimelineStatId(pub String);

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
