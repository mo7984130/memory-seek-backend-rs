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
    /// 根据照片创建时间批量递减时间线统计计数
    ///
    /// 将创建时间按年月分组后，使用 `CASE WHEN` 表达式批量更新对应月份的计数，
    /// 并通过 `GREATEST(..., 0)` 保证计数不低于零。
    ///
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `created_ats`: 照片创建时间列表
    ///
    /// # 错误
    /// - `AppError`: 数据库更新失败
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
