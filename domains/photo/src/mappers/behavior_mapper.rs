use chrono::{DateTime, Utc};
use common::error::{AppError, deferred::Result};
use common::ext::{DeferResultExt, ToOk};
use sea_orm::entity::prelude::Json;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use types::auth::user::UserId;
use types::cursor::TimeIdCursor;
use types::photo::behavior::*;

pub(crate) struct BehaviorMapper;

// 写入
impl BehaviorMapper {
    /// 插入一条行为审计记录（只追加，永不删除）
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        db: &impl ConnectionTrait,
        user_id: UserId,
        action: UserBehaviorAction,
        target_type: Option<BehaviorTargetType>,
        target_id: Option<i64>,
        detail: Option<Json>,
        ip: Option<&str>,
    ) -> Result<()> {
        Entity::insert(ActiveModel {
            user_id: Set(user_id),
            action: Set(action.as_str().to_string()),
            target_type: Set(target_type.map(|t| t.as_str().to_string())),
            target_id: Set(target_id),
            detail: Set(detail),
            ip: Set(ip.map(String::from)),
            created_at: Set(Utc::now()),
            ..Default::default()
        })
        .exec(db)
        .await
        .defer_error(
            "db_insert_err",
            "插入行为审计记录失败",
            AppError::InternalServerError,
        )?;
        Ok(())
    }
}

// 管理端聚合查询
impl BehaviorMapper {
    /// 按时间桶统计行为量
    ///
    /// 返回 (时间桶, 数量) 列表，`start`/`end` 为闭区间过滤。
    #[allow(clippy::too_many_arguments)]
    pub async fn query_stats(
        db: &impl ConnectionTrait,
        action: Option<UserBehaviorAction>,
        target_type: Option<BehaviorTargetType>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        trunc: &str,
    ) -> Result<Vec<(DateTime<Utc>, i64)>> {
        let mut sql = format!(
            "SELECT date_trunc('{trunc}', created_at) AS bucket, COUNT(*) AS cnt \
             FROM photo_user_behavior WHERE 1 = 1"
        );
        let mut binds: Vec<sea_orm::Value> = Vec::new();

        if let Some(action) = action {
            sql.push_str(" AND action = $");
            sql.push_str(&(binds.len() + 1).to_string());
            binds.push(action.as_str().into());
        }
        if let Some(target_type) = target_type {
            sql.push_str(" AND target_type = $");
            sql.push_str(&(binds.len() + 1).to_string());
            binds.push(target_type.as_str().into());
        }
        if let Some(start) = start {
            sql.push_str(" AND created_at >= $");
            sql.push_str(&(binds.len() + 1).to_string());
            binds.push(start.into());
        }
        if let Some(end) = end {
            sql.push_str(" AND created_at <= $");
            sql.push_str(&(binds.len() + 1).to_string());
            binds.push(end.into());
        }

        sql.push_str(" GROUP BY bucket ORDER BY bucket ASC");

        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, &sql, binds);
        let rows = db.query_all(stmt).await.defer_error(
            "db_query_err",
            "查询行为量统计失败",
            AppError::InternalServerError,
        )?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let bucket: DateTime<Utc> = row.try_get("", "bucket")?;
            let cnt: i64 = row.try_get("", "cnt")?;
            result.push((bucket, cnt));
        }
        result.to_ok()
    }

    /// 热门目标排行（如浏览量 Top N 照片）
    pub async fn query_top_targets(
        db: &impl ConnectionTrait,
        action: UserBehaviorAction,
        target_type: BehaviorTargetType,
        limit: u64,
    ) -> Result<Vec<(i64, i64)>> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT target_id, COUNT(*) AS cnt FROM photo_user_behavior \
             WHERE action = $1 AND target_type = $2 AND target_id IS NOT NULL \
             GROUP BY target_id ORDER BY cnt DESC, target_id DESC LIMIT $3",
            [
                action.as_str().into(),
                target_type.as_str().into(),
                limit.into(),
            ],
        );

        let rows = db.query_all(stmt).await.defer_error(
            "db_query_err",
            "查询热门目标排行失败",
            AppError::InternalServerError,
        )?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let target_id: i64 = row.try_get("", "target_id")?;
            let cnt: i64 = row.try_get("", "cnt")?;
            result.push((target_id, cnt));
        }
        result.to_ok()
    }

    /// 审计流水分页查询（(created_at, id) keyset 倒序，多查 1 条用于 has_more 判定）
    #[allow(clippy::too_many_arguments)]
    pub async fn query_audit_page(
        db: &impl ConnectionTrait,
        action: Option<UserBehaviorAction>,
        target_type: Option<BehaviorTargetType>,
        target_id: Option<i64>,
        user_id: Option<UserId>,
        cursor: &Option<TimeIdCursor<UserBehaviorId>>,
        size: u64,
    ) -> Result<Vec<BehaviorRecord>> {
        let mut query = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(size + 1);

        if let Some(action) = action {
            query = query.filter(Column::Action.eq(action.as_str()));
        }
        if let Some(target_type) = target_type {
            query = query.filter(Column::TargetType.eq(target_type.as_str()));
        }
        if let Some(target_id) = target_id {
            query = query.filter(Column::TargetId.eq(target_id));
        }
        if let Some(user_id) = user_id {
            query = query.filter(Column::UserId.eq(user_id));
        }
        if let Some(c) = cursor {
            query = query.filter(c.before(Column::CreatedAt, Column::Id));
        }

        query
            .all(db)
            .await
            .defer_error(
                "db_query_err",
                "查询审计流水失败",
                AppError::InternalServerError,
            )?
            .into_iter()
            .map(BehaviorRecord::from)
            .collect::<Vec<_>>()
            .to_ok()
    }
}
