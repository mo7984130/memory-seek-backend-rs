use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use entities::timeline_stat;
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::models::common::PhotoTimelineStatVO;
use crate::state::PhotoState;

pub struct TimelineStatService;

impl TimelineStatService {
    /// 递增时间线统计（委托给事务版本）
    ///
    /// # 参数
    /// - `state`: 照片域状态
    /// - `created_at`: 照片创建时间
    ///
    /// # 错误
    /// - `AppError`: 更新统计失败
    pub async fn incr_stat(
        state: &PhotoState,
        created_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        Self::incr_stat_txn(&state.db, created_at).await
    }

    /// 递减时间线统计（委托给事务版本）
    ///
    /// # 参数
    /// - `state`: 照片域状态
    /// - `created_at`: 照片创建时间
    ///
    /// # 错误
    /// - `AppError`: 更新统计失败
    pub async fn decr_stat(
        state: &PhotoState,
        created_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        Self::decr_stat_txn(&state.db, created_at).await
    }

    /// 递增时间线统计（支持事务，使用 upsert 保证原子性）
    ///
    /// # 参数
    /// - `db`: 数据库连接（支持事务）
    /// - `created_at`: 照片创建时间
    ///
    /// # 错误
    /// - `AppError`: 更新统计失败
    pub async fn incr_stat_txn<C: ConnectionTrait>(
        db: &C,
        created_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let date_str = created_at.format("%Y-%m").to_string();
        let now = Utc::now();

        let insert = timeline_stat::ActiveModel {
            date_str: Set(date_str),
            count: Set(1),
            anchor_time: Set(created_at.into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let mut on_conflict = OnConflict::column(timeline_stat::Column::DateStr);
        on_conflict
            .update_columns([
                timeline_stat::Column::AnchorTime,
                timeline_stat::Column::UpdatedAt,
            ])
            .value(
                timeline_stat::Column::Count,
                Expr::col(timeline_stat::Column::Count).add(1),
            );

        timeline_stat::Entity::insert(insert)
            .on_conflict(on_conflict)
            .exec(db)
            .await
            .map_internal_err("更新统计失败")?;

        Ok(())
    }

    /// 递减时间线统计（支持事务）
    ///
    /// count > 1 时原子递减，count <= 1 时删除记录。
    ///
    /// # 参数
    /// - `db`: 数据库连接（支持事务）
    /// - `created_at`: 照片创建时间
    ///
    /// # 错误
    /// - `AppError`: 查询或更新统计失败
    pub async fn decr_stat_txn<C: ConnectionTrait>(
        db: &C,
        created_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let date_str = created_at.format("%Y-%m").to_string();

        let existing = timeline_stat::Entity::find()
            .filter(timeline_stat::Column::DateStr.eq(&date_str))
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        let Some(stat) = existing else {
            return Ok(());
        };

        if stat.count <= 1 {
            timeline_stat::Entity::delete_by_id(stat.date_str)
                .exec(db)
                .await
                .map_internal_err("删除统计失败")?;
        } else {
            // 使用 upsert 原子递减，避免 check-then-act 竞态
            let insert = timeline_stat::ActiveModel {
                date_str: Set(date_str),
                count: Set(0), // INSERT 时的占位值，实际走 UPDATE 分支
                anchor_time: Set(stat.anchor_time),
                created_at: Set(stat.created_at),
                updated_at: Set(Utc::now().into()),
            };

            let mut on_conflict = OnConflict::column(timeline_stat::Column::DateStr);
            on_conflict
                .update_columns([timeline_stat::Column::UpdatedAt])
                .value(
                    timeline_stat::Column::Count,
                    Expr::col(timeline_stat::Column::Count).sub(1),
                );

            timeline_stat::Entity::insert(insert)
                .on_conflict(on_conflict)
                .exec(db)
                .await
                .map_internal_err("更新统计失败")?;
        }

        Ok(())
    }

    /// 获取时间线统计列表
    ///
    /// # 参数
    /// - `state`: 照片域状态
    ///
    /// # 返回
    /// 返回按日期倒序排列的时间线统计列表
    ///
    /// # 错误
    /// - `AppError`: 查询统计失败
    pub async fn get_stats(
        state: &PhotoState,
    ) -> Result<Vec<PhotoTimelineStatVO>, AppError> {
        let stats = timeline_stat::Entity::find()
            .order_by_desc(timeline_stat::Column::DateStr)
            .all(&state.db)
            .await
            .map_internal_err("查询失败")?;

        Ok(stats
            .iter()
            .map(|s| PhotoTimelineStatVO {
                date_str: s.date_str.clone(),
                count: s.count,
                anchor_time: s.anchor_time.with_timezone(&Utc),
            })
            .collect())
    }
}
