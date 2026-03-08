use chrono::{DateTime, Utc};
use common::error::AppError;
use common::utils::ResultExt;
use entities::timeline_stat;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::models::common::PhotoTimelineStatVO;

pub struct TimelineStatService;

impl TimelineStatService {
    pub async fn incr_stat(
        db: &DatabaseConnection,
        created_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let date_str = created_at.format("%Y-%m").to_string();

        let existing: Option<timeline_stat::Model> = timeline_stat::Entity::find()
            .filter(timeline_stat::Column::DateStr.eq(&date_str))
            .one(db)
            .await
            .map_internal_err("查询失败")?;

        if let Some(stat) = existing {
            let mut active: timeline_stat::ActiveModel = stat.into();
            active.count = Set(active.count.unwrap() + 1);
            active.anchor_time = Set(created_at.into());
            active.updated_at = Set(Utc::now().into());
            let _ = active.update(db).await;
        } else {
            let now = Utc::now();
            let stat = timeline_stat::ActiveModel {
                date_str: Set(date_str),
                count: Set(1),
                anchor_time: Set(created_at.into()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            };
            let _ = stat.insert(db).await;
        }

        Ok(())
    }

    pub async fn get_stats(
        db: &DatabaseConnection,
    ) -> Result<Vec<PhotoTimelineStatVO>, AppError> {
        let stats = timeline_stat::Entity::find()
            .order_by_desc(timeline_stat::Column::DateStr)
            .all(db)
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
