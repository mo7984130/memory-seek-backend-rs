use common_core::time::DateTime;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "admin_job")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "String(StringLen::N(36))")]
    pub id: String,
    #[sea_orm(column_type = "String(StringLen::N(64))")]
    pub kind: String,
    #[sea_orm(column_type = "String(StringLen::N(32))")]
    pub status: String,
    pub requested_by: i64,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub result: Option<Json>,
    #[sea_orm(column_type = "String(StringLen::N(2048))", nullable)]
    pub error: Option<String>,
    pub created_at: DateTime,
    pub started_at: Option<DateTime>,
    pub finished_at: Option<DateTime>,
    pub cancel_requested: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
