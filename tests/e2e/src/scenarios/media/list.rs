//! 媒体列表 / MD5 查存在 / 时间线统计。

use common::axum::{ErrR, SucR};
use common::types::CursorPage;
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use types::cursor::TimeIdCursor;
use types::media::dto::media::MediaView;
use types::media::dto::timeline_stat::MonthStat;
use types::media::media as media_entity;
use types::media::media::MediaId;

use crate::context::Context;

use super::{Session, db_current_month, seed_media, session, token_viewer};

/// 分页校验用页大小。
const PAGE_SIZE: u64 = 10;

/// 游标分页前置:登录 + 以最旧种子媒体构造游标(走 keyset 分支, 不依赖首屏缓存)。
#[derive(Default)]
pub struct CursorSetup {
    pub session: Session,
    pub cursor: String,
}

/// 获取媒体游标页: 走 `direction=prev` + 游标(keyset)分支, 校验分页契约。
#[derive(Default)]
pub struct GetMediasCursorScenario;

impl Scenario for GetMediasCursorScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CursorPage<MediaView, TimeIdCursor<MediaId>>>;

    type Setup = CursorSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let oldest = media_entity::Entity::find()
            .filter(media_entity::Column::FileId.like("seed_file_%"))
            .order_by_asc(media_entity::Column::CreatedAt)
            .order_by_asc(media_entity::Column::Id)
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
            .request(reqwest::Method::GET, "/media")
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

register_scenario!(GetMediasCursorScenario);

/// 未携带认证头: 期望 401。
#[derive(Default)]
pub struct GetMediasCursorUnauthorizedScenario;

impl Scenario for GetMediasCursorUnauthorizedScenario {
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
            .request(reqwest::Method::GET, "/media")
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

register_scenario!(GetMediasCursorUnauthorizedScenario);

/// 不存在的 MD5 占位(种子 md5 为 32 位纯数字, 不会与之相等)。
const MISSING_MD5: &str = "e2e_missing_md5_000000000000000000";

/// MD5 查存在前置:登录 + 取一张种子媒体的真实 md5。
#[derive(Default)]
pub struct Md5Setup {
    pub session: Session,
    pub md5: String,
}

/// 批量查 MD5: 种子媒体命中、随机 MD5 落空。
#[derive(Default)]
pub struct Md5sExistScenario;

impl Scenario for Md5sExistScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<Vec<bool>>;

    type Setup = Md5Setup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let media = seed_media(ctx, 1).await.ok_or_else(super::seed_missing)?;
        Ok(Md5Setup {
            session,
            md5: media.md5,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/media/check-existence")
            .header("Authorization", &setup.session.auth_header())
            .json_unwrap(&serde_json::json!({ "md5s": [setup.md5, MISSING_MD5] }))
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

register_scenario!(Md5sExistScenario);

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
            .request(reqwest::Method::GET, "/media/timeline/stats")
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
