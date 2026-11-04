use std::collections::HashMap;

use common_core::{
    DbConn as ConnectionTrait,
    error::contextual::Result,
    error::contextual::ext::IntoContextualExt,
    time::{DateTime, now},
};
use sea_orm::{
    ActiveValue::Set,
    ColumnTrait, DatabaseTransaction, DbBackend, EntityTrait, ExprTrait, QueryFilter, QueryOrder,
    QuerySelect, Statement,
    sea_query::{Alias, Expr, Func, OnConflict},
};
use types_visual::timeline_stat::TimelineStatId;
use types_visual::{
    dto::timeline_stat::MonthStat,
    timeline_stat::*,
    visual::{Column as VisualColumn, Entity as VisualEntity, VisualId, VisualRecord},
};

pub(crate) struct TimelineStatMapper;

/// 时间线统计锁命名空间(ASCII "TIME"), 与 backup 的单 bigint 锁区分。
const TIMELINE_STAT_LOCK_NAMESPACE: i32 = 0x5449_4D45;

impl TimelineStatMapper {
    /// 串行化同一月份的时间线统计写入。
    ///
    /// 上传走 `count + 1`, 删除走「按剩余影像重算」, 两者都是读改写:
    /// 如果只有一侧加锁, 重算会覆盖并发上传的增量, 反之亦然。
    ///
    /// 必须传入事务: `pg_advisory_xact_lock` 在事务结束时释放, 用 autocommit
    /// 连接会在语句结束就失去保护。
    ///
    /// 锁 key 用 `(命名空间, hashtext(YYYY-MM))` 两参数形式, 不同月份互不阻塞;
    /// hash 碰撞只会让两个月份排队, 不影响正确性。
    async fn lock_month(txn: &DatabaseTransaction, date_str: &str) -> Result<()> {
        txn.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1, hashtext($2))",
            [
                TIMELINE_STAT_LOCK_NAMESPACE.into(),
                date_str.to_owned().into(),
            ],
        ))
        .await?;
        Ok(())
    }

    /// 将指定月份的影像统计增加一个单位
    pub async fn incr_stat(txn: &DatabaseTransaction, created_at: DateTime) -> Result<()> {
        let date_str = to_date_str(&created_at);
        Self::lock_month(txn, &date_str).await?;

        let updated_at = now();
        // 单条 upsert: 不再有「UPDATE 0 行 → INSERT」的主键竞态窗口。
        txn.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO visual_timeline_stat (date_str, count, anchor_time, created_at, updated_at)
            VALUES ($1, 1, $2, $3, $3)
            ON CONFLICT (date_str) DO UPDATE
            SET count       = visual_timeline_stat.count + 1,
                anchor_time = GREATEST(visual_timeline_stat.anchor_time, EXCLUDED.anchor_time),
                updated_at  = EXCLUDED.updated_at
            "#,
            [date_str.into(), created_at.into(), updated_at.into()],
        ))
        .await?;

        Ok(())
    }

    /// 删除影像后按剩余影像重建受影响月份的统计
    pub async fn refresh_after_delete(
        txn: &DatabaseTransaction,
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

        // 一次批量删除可能跨多个月: 固定按月份升序加锁, 避免两个批量删除
        // 分别持有 A/B 月时形成 ABBA 死锁。
        let mut date_visual_ids: Vec<(String, Vec<VisualId>)> =
            date_visual_ids.into_iter().collect();
        date_visual_ids.sort_by(|left, right| left.0.cmp(&right.0));

        for (date_str, deleted_visual_ids) in date_visual_ids {
            Self::lock_month(txn, &date_str).await?;

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
                .one(txn)
                .await?
                .unwrap_or((0, None));

            let (count, anchor_time) = remaining;
            let Some(anchor_time) = anchor_time else {
                // 该月已无剩余影像，删除统计行（即使原计数已经不准确）。
                Entity::delete_many()
                    .filter(Column::DateStr.eq(TimelineStatId(date_str)))
                    .exec(txn)
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
            .exec_without_returning(txn)
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
