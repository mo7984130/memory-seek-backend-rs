use std::collections::HashMap;

use common::{
    error::contextual::Result,
    ext::IntoContextualExt,
    time::{DateTime, now},
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Alias, CaseStatement, Expr, Func, SimpleExpr},
};
use types::photo::timeline_stat::TimelineStatId;
use types::photo::{dto::timeline_stat::MonthStat, timeline_stat::*};

pub(crate) struct TimelineStatMapper;

impl TimelineStatMapper {
    /// 将指定月份的照片统计增加一个单位.
    pub async fn incr_stat(db: &impl ConnectionTrait, created_at: DateTime) -> Result<()> {
        let date_str = to_date_str(&created_at);
        // 先更新
        let result = Entity::update_many()
            .col_expr(Column::Count, Expr::col(Column::Count).add(1))
            .col_expr(
                Column::AnchorTime,
                Expr::expr(Func::greatest([
                    Expr::col(Column::AnchorTime).into(),
                    Expr::value(created_at),
                ]))
                .into(),
            )
            .col_expr(Column::UpdatedAt, Expr::value(now()))
            .filter(Column::DateStr.eq(date_str))
            .exec(db)
            .await?;

        if result.rows_affected != 0 {
            return Ok(());
        }

        // 如果更新失败, 插入
        Self::insert(db, created_at).await
    }

    async fn insert(db: &impl ConnectionTrait, created_at: DateTime) -> Result<()> {
        let date_str = to_date_str(&created_at);
        let now = now();
        ActiveModel {
            date_str: Set(TimelineStatId(date_str)),
            count: Set(1),
            anchor_time: Set(created_at),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await?;

        Ok(())
    }

    /// 按照片创建时间批量扣减月份统计.
    pub async fn decr_by_created_ats(
        db: &impl ConnectionTrait,
        created_ats: &[&DateTime],
    ) -> Result<()> {
        let mut date_count_map: HashMap<String, i64> = HashMap::new();
        for created_at in created_ats {
            let date_str = created_at.format("%Y-%m").to_string();
            *date_count_map.entry(date_str).or_insert(0) += 1;
        }

        // 构建 CASE WHEN date_str = 'x' THEN n ... END
        let mut case_expr = CaseStatement::new();
        let mut date_strs = Vec::new();

        for (date_str, decr_count) in date_count_map {
            let id = TimelineStatId(date_str);
            case_expr = case_expr.case(
                Expr::col(Column::DateStr).eq(id.clone()),
                Expr::col(Column::Count).sub(decr_count),
            );
            date_strs.push(id);
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
            .await?;

        Ok(())
    }

    /// 查询全部月份统计并按月份排序.
    pub async fn query_monthly_stats(db: &impl ConnectionTrait) -> Result<Vec<MonthStat>> {
        let result = Entity::find()
            .select_only()
            .column(Column::DateStr)
            .column(Column::Count)
            .order_by_asc(Column::DateStr)
            .into_model::<MonthStat>()
            .all(db)
            .await
            .into_contextual()?;
        Ok(result)
    }
}
