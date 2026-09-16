//! 评论:`/visual/comment`。

use common::axum::{ErrR, SucR};
use common::types::CursorPage;
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types::cursor::TimeIdCursor;
use types::visual::comment as comment_entity;
use types::visual::comment::CommentId;
use types::visual::dto::comment::CommentView;
use types::visual::visual as visual_entity;
use types::visual::visual::VisualId;

use crate::context::Context;

use super::{Session, seed_visual, session};

/// 在指定影像下发布评论, 返回视图。
async fn publish(
    ctx: &Context,
    session: &Session,
    visual_id: VisualId,
    content: &str,
) -> Result<SucR<CommentView>, HttpError> {
    ctx.client
        .request(reqwest::Method::POST, &format!("/visual/comment/{visual_id}"))
        .header("Authorization", &session.auth_header())
        .json_unwrap(&json!({ "content": content }))
        .send_checked()
        .await?
        .json::<SucR<CommentView>>()
        .await
        .map_err(HttpError::from)
}

/// 评论前置:登录 + 目标影像(种子影像 u=6)。
/// id 以 i64 保存, 便于 `Default`(强类型 ID 不实现 `Default`)。
#[derive(Default)]
pub struct CommentSetup {
    pub session: Session,
    pub visual_id: i64,
}

async fn comment_setup(ctx: &Context, task: &TaskIndex) -> Result<CommentSetup, HttpError> {
    let session = session(ctx, task.index).await?;
    let visual = seed_visual(ctx, 6).await.ok_or_else(super::seed_missing)?;
    Ok(CommentSetup {
        session,
        visual_id: visual.id.0,
    })
}

/// 发布评论: 落库归属/内容正确, 影像评论数递增。
#[derive(Default)]
pub struct PublishCommentScenario;

impl Scenario for PublishCommentScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CommentView>;

    type Setup = CommentSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        comment_setup(ctx, task).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        publish(ctx, &setup.session, VisualId(setup.visual_id), "e2e comment").await
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let Some(row) = comment_entity::Entity::find_by_id(output.data.id)
            .one(&ctx.db)
            .await
            .unwrap()
        else {
            return Ok(false);
        };
        let row_ok = row.user_id == setup.session.user_id
            && row.visual_id == VisualId(setup.visual_id)
            && row.content == "e2e comment";
        let visual_ok = visual_entity::Entity::find_by_id(VisualId(setup.visual_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some_and(|p| p.comment_count > 0);
        Ok(row_ok && visual_ok)
    }
}

register_scenario!(PublishCommentScenario);

/// 空内容评论: 期望 400(参数校验失败)。
#[derive(Default)]
pub struct PublishCommentEmptyScenario;

impl Scenario for PublishCommentEmptyScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = CommentSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        comment_setup(ctx, task).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/visual/comment/{}", setup.visual_id),
            )
            .header("Authorization", &setup.session.auth_header())
            .json_unwrap(&json!({ "content": "" }))
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

register_scenario!(PublishCommentEmptyScenario);

/// 评论列表前置:登录 + 目标影像 + 前置评论。
#[derive(Default)]
pub struct CommentListSetup {
    pub session: Session,
    pub visual_id: i64,
    pub comment_id: i64,
}

/// 评论游标列表: 包含前置发表的评论。
#[derive(Default)]
pub struct GetCommentsScenario;

impl Scenario for GetCommentsScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CursorPage<CommentView, TimeIdCursor<CommentId>>>;

    type Setup = CommentListSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let base = comment_setup(ctx, task).await?;
        let comment = publish(
            ctx,
            &base.session,
            VisualId(base.visual_id),
            "e2e list comment",
        )
        .await?
        .data;
        Ok(CommentListSetup {
            session: base.session,
            visual_id: base.visual_id,
            comment_id: comment.id.0,
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
                &format!("/visual/comment/{}", setup.visual_id),
            )
            .header("Authorization", &setup.session.auth_header())
            .query("size", "50")
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
        Ok(output
            .data
            .records
            .iter()
            .any(|c| c.id.0 == setup.comment_id))
    }
}

register_scenario!(GetCommentsScenario);

/// 删除评论前置:登录 + 目标影像 + 前置评论。
#[derive(Default)]
pub struct CommentDeleteSetup {
    pub session: Session,
    pub visual_id: i64,
    pub comment_id: i64,
}

/// 删除评论: 评论记录消失。
#[derive(Default)]
pub struct DeleteCommentScenario;

impl Scenario for DeleteCommentScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = CommentDeleteSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let base = comment_setup(ctx, task).await?;
        let comment = publish(
            ctx,
            &base.session,
            VisualId(base.visual_id),
            "e2e delete comment",
        )
        .await?
        .data;
        Ok(CommentDeleteSetup {
            session: base.session,
            visual_id: base.visual_id,
            comment_id: comment.id.0,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(
                reqwest::Method::DELETE,
                &format!("/visual/comment/{}/{}", setup.visual_id, setup.comment_id),
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
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let gone = comment_entity::Entity::find()
            .filter(comment_entity::Column::Id.eq(CommentId(setup.comment_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(DeleteCommentScenario);
