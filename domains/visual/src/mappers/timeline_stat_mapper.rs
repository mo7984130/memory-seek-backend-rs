use std::collections::HashMap;

use common_core::{
    DbConn as ConnectionTrait,
    error::contextual::Result,
    error::contextual::ext::IntoContextualExt,
    time::{DateTime, now},
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, EntityTrait, ExprTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Alias, Expr, Func, OnConflict},
};
use types_visual::timeline_stat::TimelineStatId;
use types_visual::{
    dto::timeline_stat::MonthStat,
    timeline_stat::*,
    visual::{Column as VisualColumn, Entity as VisualEntity, VisualId, VisualRecord},
};

pub(crate) struct TimelineStatMapper;

impl TimelineStatMapper {
    /// 将指定月份的影像统计增加一个单位
    pub async fn incr_stat(db: &impl ConnectionTrait, created_at: DateTime) -> Result<()> {
        let date_str = to_date_str(&created_at);
        // 先更新
        let result = Entity::update_many()
            .col_expr(Column::Count, Expr::col(Column::Count).add(1))
            .col_expr(
                Column::AnchorTime,
                Expr::expr(Func::greatest([
                    Expr::col(Column::AnchorTime),
                    Expr::value(created_at),
                ])),
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

    /// 插入一条新的月份统计记录（count 初始为 1）
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

    /// 删除影像后按剩余影像重建受影响月份的统计
    pub async fn refresh_after_delete(
        db: &impl ConnectionTrait,
        visuals: &[VisualRecord],
    ) -> Result<()> {
        let mut date_visual_ids: HashMap<String, Vec<VisualId>> = HashMap::new();
        for visual in visuals {
            let created_at = &visual.created_at;
            let date_str = created_at.format("%Y-%m").to_string();
            date_visual_ids.entry(date_str).or_default().push(visual.id);
        }

        if date_visual_ids.is_empty() {
            return Ok(());
        }

        for (date_str, deleted_visual_ids) in date_visual_ids {
            // 删除步骤在主表删除之前执行，因此显式排除本次待删除的影像。
            // 直接基于剩余影像重算，确保 count 和 anchor_time 同步准确。
            let remaining = VisualEntity::find()
                .select_only()
                .column_as(VisualColumn::Id.count(), "count")
                .expr_as(
                    Func::cust(Alias::new("max"))
                        .arg(Expr::col((VisualEntity, VisualColumn::CreatedAt))),
                    "anchor_time",
                )
                .filter(VisualColumn::Id.is_not_in(deleted_visual_ids))
                .filter(
                    Expr::cust("to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM')")
                        .eq(date_str.as_str()),
                )
                .into_tuple::<(i64, Option<DateTime>)>()
                .one(db)
                .await?
                .unwrap_or((0, None));

            let (count, anchor_time) = remaining;
            let Some(anchor_time) = anchor_time else {
                // 该月已无剩余影像，删除统计行（即使原计数已经不准确）。
                Entity::delete_many()
                    .filter(Column::DateStr.eq(TimelineStatId(date_str)))
                    .exec(db)
                    .await?;
                continue;
            };

            let now = now();
            Entity::insert(ActiveModel {
                date_str: Set(TimelineStatId(date_str)),
                count: Set(count),
                anchor_time: Set(anchor_time),
                created_at: Set(now),
                updated_at: Set(now),
            })
            .on_conflict(
                OnConflict::column(Column::DateStr)
                    .update_columns([Column::Count, Column::AnchorTime, Column::UpdatedAt])
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
        }

        Ok(())
    }

    /// 查询全部月份统计并按月份排序
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
