use std::collections::HashMap;

use chrono::Utc;
use common::{Result, ext::ResultErrExt};
use entities::photo::timeline_stat::*;
use sea_orm::{
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect,
    entity::prelude::DateTimeUtc,
    sea_query::{Alias, CaseStatement, Expr, Func, OnConflict, SimpleExpr},
};

use crate::models::timeline_stat::TimeRange;

pub(crate) struct TimelineStatMapper;

impl TimelineStatMapper {
    pub async fn incr_stat(db: &impl ConnectionTrait, created_at: DateTimeUtc) -> Result<()> {
        let date_str = created_at.format("%Y-%m").to_string();
        let now = Utc::now();

        let insert = ActiveModel {
            date_str: Set(date_str),
            count: Set(1),
            anchor_time: Set(created_at),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let mut on_conflict = OnConflict::column(Column::DateStr);
        on_conflict
            .update_columns([Column::AnchorTime, Column::UpdatedAt])
            .value(Column::Count, Expr::col(Column::Count).add(1));

        Entity::insert(insert)
            .on_conflict(on_conflict)
            .exec(db)
            .await
            .trace_internal_err("db_update_err", "更新照片时间线统计失败")?;

        Ok(())
    }

    pub async fn decr_stat_by_created_ats(
        db: &impl ConnectionTrait,
        created_ats: &[DateTimeUtc],
    ) -> Result<()> {
        let mut date_count_map: HashMap<String, i64> = HashMap::new();
        for created_at in created_ats {
            let date_str = created_at.format("%Y-%m").to_string();
            *date_count_map.entry(date_str).or_insert(0) += 1;
        }

        if date_count_map.is_empty() {
            return Ok(());
        }

        // 构建 CASE WHEN date_str = 'x' THEN n ... END
        let mut case_expr = CaseStatement::new();
        let mut date_strs = Vec::new();

        for (date_str, decr_count) in &date_count_map {
            case_expr = case_expr.case(
                Expr::col(Column::DateStr).eq(date_str.clone()),
                Expr::col(Column::Count).sub(*decr_count),
            );
            date_strs.push(date_str.clone());
        }
        // ELSE count (不在列表中的行保持不变，实际上 filter 已经限制了)
        case_expr = case_expr.finally(Expr::col(Column::Count));

        Entity::update_many()
            .col_expr(
                Column::Count,
                // GREATEST(CASE WHEN ... END, 0)
                Func::cust(Alias::new("GREATEST"))
                    .arg(SimpleExpr::Case(Box::new(case_expr)))
                    .arg(0i64)
                    .into(),
            )
            .col_expr(Column::UpdatedAt, Expr::current_timestamp().into())
            .filter(Column::DateStr.is_in(date_strs))
            .exec(db)
            .await
            .trace_internal_err("db_update_err", "批量更新照片时间线统计错误")?;

        Ok(())
    }

    pub async fn query_time_range(db: &impl ConnectionTrait) -> Result<TimeRange> {
        let result = Entity::find()
            .select_only()
            .column_as(Column::CreatedAt.min(), "min_time")
            .column_as(Column::CreatedAt.max(), "max_time")
            .into_model::<TimeRange>()
            .one(db)
            .await
            .trace_internal_err("db_query_err", "查询时间范围失败")?;
        Ok(result.unwrap_or_default())
    }
}
