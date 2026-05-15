use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use entities::timeline_stat;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::models::common::PhotoTimelineStatVO;
use crate::state::PhotoState;

pub struct TimelineStatService;

impl TimelineStatService {
    /// 递增时间线统计
    /// 
    /// 使用原子更新方式递增计数
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `created_at`: 照片创建时间
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn incr_stat(
        state: &PhotoState,
        created_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let date_str = created_at.format("%Y-%m").to_string();

        let existing = timeline_stat::Entity::find()
            .filter(timeline_stat::Column::DateStr.eq(&date_str))
            .one(&state.db)
            .await
            .map_internal_err("查询失败")?;

        if let Some(stat) = existing {
            let mut active: timeline_stat::ActiveModel = stat.into();
            active.count = Set(active.count.unwrap() + 1);
            active.anchor_time = Set(created_at.into());
            active.updated_at = Set(Utc::now().into());
            active.update(&state.db).await.map_internal_err("更新统计失败")?;
        } else {
            let now = Utc::now();
            let stat = timeline_stat::ActiveModel {
                date_str: Set(date_str),
                count: Set(1),
                anchor_time: Set(created_at.into()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            };
            stat.insert(&state.db).await.map_internal_err("创建统计失败")?;
        }

        Ok(())
    }

    /// 递减时间线统计
    /// 
    /// 使用原子更新方式递减计数
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// - `created_at`: 照片创建时间
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn decr_stat(
        state: &PhotoState,
        created_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let date_str = created_at.format("%Y-%m").to_string();

        let existing = timeline_stat::Entity::find()
            .filter(timeline_stat::Column::DateStr.eq(&date_str))
            .one(&state.db)
            .await
            .map_internal_err("查询失败")?;

        if let Some(stat) = existing {
            if stat.count > 1 {
                let mut active: timeline_stat::ActiveModel = stat.into();
                active.count = Set(active.count.unwrap() - 1);
                active.updated_at = Set(Utc::now().into());
                active.update(&state.db).await.map_internal_err("更新统计失败")?;
            } else {
                timeline_stat::Entity::delete_by_id(stat.date_str)
                    .exec(&state.db)
                    .await
                    .map_internal_err("删除统计失败")?;
            }
        }

        Ok(())
    }

    /// 递增时间线统计（支持事务）
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `created_at`: 照片创建时间
    /// 
    /// # 返回
    /// 成功返回空元组
    pub async fn incr_stat_txn<C: ConnectionTrait>(
        db: &C,
        created_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let date_str = created_at.format("%Y-%m").to_string();

        let existing = timeline_stat::Entity::find()
            .filter(timeline_stat::Column::DateStr.eq(&date_str))
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        if let Some(stat) = existing {
            let mut active: timeline_stat::ActiveModel = stat.into();
            active.count = Set(active.count.unwrap() + 1);
            active.anchor_time = Set(created_at.into());
            active.updated_at = Set(Utc::now().into());
            active.update(db).await.map_internal_err("更新统计失败")?;
        } else {
            let now = Utc::now();
            let stat = timeline_stat::ActiveModel {
                date_str: Set(date_str),
                count: Set(1),
                anchor_time: Set(created_at.into()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            };
            stat.insert(db).await.map_internal_err("创建统计失败")?;
        }

        Ok(())
    }

    /// 递减时间线统计（支持事务）
    /// 
    /// # 参数
    /// - `db`: 数据库连接或事务
    /// - `created_at`: 照片创建时间
    /// 
    /// # 返回
    /// 成功返回空元组
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

        if let Some(stat) = existing {
            if stat.count > 1 {
                let mut active: timeline_stat::ActiveModel = stat.into();
                active.count = Set(active.count.unwrap() - 1);
                active.updated_at = Set(Utc::now().into());
                active.update(db).await.map_internal_err("更新统计失败")?;
            } else {
                timeline_stat::Entity::delete_by_id(stat.date_str)
                    .exec(db)
                    .await
                    .map_internal_err("删除统计失败")?;
            }
        }

        Ok(())
    }

    /// 获取时间线统计列表
    /// 
    /// # 参数
    /// - `db`: 数据库连接
    /// 
    /// # 返回
    /// 返回按日期倒序排列的统计列表
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
