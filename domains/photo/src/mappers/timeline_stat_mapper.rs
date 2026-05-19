use std::collections::HashMap;

use common::{error::AppError, utils::ResultExt};
use entities::timeline_stat::{Column, Entity};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    entity::prelude::DateTimeUtc,
    sea_query::{Alias, CaseStatement, Expr, Func, SimpleExpr},
};

pub struct TimelineStatMapper;

impl TimelineStatMapper {
    pub async fn decr_stat_by_created_ats(
        db: &impl ConnectionTrait,
        created_ats: Vec<DateTimeUtc>,
    ) -> Result<(), AppError> {
        let mut date_count_map: HashMap<String, i64> = HashMap::new();
        for created_at in &created_ats {
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
}
