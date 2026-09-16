//! 影像详情:`GET /visual/visual/{visual_id}`。

use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::Scenario,
};
use sea_orm::EntityTrait;
use types::visual::dto::visual::VisualView;
use types::visual::visual as visual_entity;
use types::visual::visual::VisualId;

use crate::context::Context;

use super::{Session, seed_visual, session, token_matches};

/// 详情前置:登录 + 定位一张种子影像。
/// id 以 i64 保存, 便于 `Default`(强类型 ID 不实现 `Default`)。
#[derive(Default)]
pub struct DetailSetup {
    pub session: Session,
    pub visual_id: i64,
    pub file_id: String,
}

/// 获取影像详情: 响应与库中记录一致, token 绑定当前浏览者。
#[derive(Default)]
pub struct GetVisualInfoScenario;

impl Scenario for GetVisualInfoScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<VisualView>;

    type Setup = DetailSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 1).await.ok_or_else(super::seed_missing)?;
        Ok(DetailSetup {
            session,
            visual_id: visual.id.0,
            file_id: visual.file_id,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(
                reqwest::Method::GET,
                &format!("/visual/visual/{}", setup.visual_id),
            )
            .header("Authorization", &setup.session.auth_header())
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let Some(row) = visual_entity::Entity::find_by_id(VisualId(setup.visual_id))
            .one(&ctx.db)
            .await
            .unwrap()
        else {
            return Ok(false);
        };
        let view = &output.data;
        let view_ok = view.id == row.id
            && view.user_id == row.user_id
            && view.name == row.name
            && view.size == row.size as u64
            && view.width == row.width as u32
            && view.height == row.height as u32;
        let viewer = setup.session.user_id;
        let tokens_ok = token_matches(view.thumbnail_token.as_ref(), &row.file_id, viewer)
            && token_matches(view.preview_token.as_ref(), &row.file_id, viewer)
            && token_matches(view.original_token.as_ref(), &row.file_id, viewer);
        Ok(view_ok && tokens_ok)
    }
}

register_scenario!(GetVisualInfoScenario);

/// 获取不存在的影像: 期望 400(影像不存在)。
#[derive(Default)]
pub struct GetVisualInfoNotFoundScenario;

impl Scenario for GetVisualInfoNotFoundScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

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
            .request(reqwest::Method::GET, "/visual/visual/999999999")
            .header("Authorization", &setup.auth_header())
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
        Ok(output.code == 400)
    }
}

register_scenario!(GetVisualInfoNotFoundScenario);
