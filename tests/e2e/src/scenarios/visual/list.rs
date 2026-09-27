//! 影像列表 / 哈希查存在 / 时间线统计。

use common_core::types::CursorPage;
use common_web::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use types_core::cursor::TimeIdCursor;
use types_visual::dto::timeline_stat::MonthStat;
use types_visual::dto::visual::VisualView;
use types_visual::visual as visual_entity;
use types_visual::visual::VisualId;

use crate::context::Context;

use super::{Session, db_current_month, seed_visual, session, token_viewer};

/// 分页校验用页大小。
const PAGE_SIZE: u64 = 10;

/// 游标分页前置:登录 + 以最旧种子影像构造游标(走 keyset 分支, 不依赖首屏缓存)。
#[derive(Default)]
pub struct CursorSetup {
    pub session: Session,
    pub cursor: String,
}

/// 获取影像游标页: 走 `direction=prev` + 游标(keyset)分支, 校验分页契约。
#[derive(Default)]
pub struct GetVisualsCursorScenario;

impl Scenario for GetVisualsCursorScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CursorPage<VisualView, TimeIdCursor<VisualId>>>;

    type Setup = CursorSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let oldest = visual_entity::Entity::find()
            .filter(visual_entity::Column::FileId.like("seed_file_%"))
            .order_by_asc(visual_entity::Column::CreatedAt)
            .order_by_asc(visual_entity::Column::Id)
            .one(&ctx.db)
            .await
            .unwrap()
            .ok_or_else(super::seed_missing)?;
        let cursor = TimeIdCursor {
            time_at: oldest.created_at,
            id: oldest.id,
        }
        .encode();
        Ok(CursorSetup { session, cursor })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::GET, "/visual")
            .header("Authorization", &setup.session.auth_header())
            .query("size", &PAGE_SIZE.to_string())
            .query("direction", "prev")
            .query("cursor", &setup.cursor)
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let page = &output.data;
        let size_ok = page.records.len() == PAGE_SIZE as usize && page.has_more;
        // 每一页记录都应带当前浏览者可用的 original token
        let viewer_ok = page
            .records
            .iter()
            .all(|p| token_viewer(p.original_token.as_ref(), setup.session.user_id));
        Ok(size_ok && viewer_ok)
    }
}

register_scenario!(GetVisualsCursorScenario);

/// 未携带认证头: 期望 401。
#[derive(Default)]
pub struct GetVisualsCursorUnauthorizedScenario;

impl Scenario for GetVisualsCursorUnauthorizedScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::GET, "/visual")
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.code == 401)
    }
}

register_scenario!(GetVisualsCursorUnauthorizedScenario);

/// 不存在的哈希占位(种子 hash 为 64 位纯数字, 不会与之相等)。
const MISSING_HASH: &str = "e2e_missing_hash_000000000000000000000000000000000000000000000000";

/// 哈希查存在前置:登录 + 取一张种子影像的真实哈希。
#[derive(Default)]
pub struct HashSetup {
    pub session: Session,
    pub hash: String,
}

/// 批量查哈希: 种子影像命中、随机哈希落空。
#[derive(Default)]
pub struct HashesExistScenario;

impl Scenario for HashesExistScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<Vec<bool>>;

    type Setup = HashSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 1).await.ok_or_else(super::seed_missing)?;
        Ok(HashSetup {
            session,
            hash: visual.hash,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/visual/check-existence")
            .header("Authorization", &setup.session.auth_header())
            .json_unwrap(&serde_json::json!({ "hashes": [setup.hash, MISSING_HASH] }))
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.data == vec![true, false])
    }
}

register_scenario!(HashesExistScenario);

/// 时间线统计: 至少包含当前月份且计数为正。
#[derive(Default)]
pub struct TimelineStatsScenario;

impl Scenario for TimelineStatsScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<Vec<MonthStat>>;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        session(ctx, task.index).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::GET, "/visual/timeline/stats")
            .header("Authorization", &setup.auth_header())
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let Some(month) = db_current_month(ctx).await else {
            return Ok(false);
        };
        Ok(output
            .data
            .iter()
            .any(|stat| stat.date_str.0 == month && stat.count > 0))
    }
}

register_scenario!(TimelineStatsScenario);
