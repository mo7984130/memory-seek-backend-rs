//! 评论:`/photo/comment`。

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
use types::photo::comment as comment_entity;
use types::photo::comment::CommentId;
use types::photo::dto::comment::CommentView;
use types::photo::photo as photo_entity;
use types::photo::photo::PhotoId;

use crate::context::Context;

use super::{Session, seed_photo, session};

/// 在指定照片下发布评论, 返回视图。
async fn publish(
    ctx: &Context,
    session: &Session,
    photo_id: PhotoId,
    content: &str,
) -> Result<SucR<CommentView>, HttpError> {
    ctx.client
        .request(reqwest::Method::POST, &format!("/photo/comment/{photo_id}"))
        .header("Authorization", &session.auth_header())
        .json_unwrap(&json!({ "content": content }))
        .send_checked()
        .await?
        .json::<SucR<CommentView>>()
        .await
        .map_err(HttpError::from)
}

/// 评论前置:登录 + 目标照片(种子照片 u=6)。
/// id 以 i64 保存, 便于 `Default`(强类型 ID 不实现 `Default`)。
#[derive(Default)]
pub struct CommentSetup {
    pub session: Session,
    pub photo_id: i64,
}

async fn comment_setup(ctx: &Context, task: &TaskIndex) -> Result<CommentSetup, HttpError> {
    let session = session(ctx, task.index).await?;
    let photo = seed_photo(ctx, 6).await.ok_or_else(super::seed_missing)?;
    Ok(CommentSetup {
        session,
        photo_id: photo.id.0,
    })
}

/// 发布评论: 落库归属/内容正确, 照片评论数递增。
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
        publish(ctx, &setup.session, PhotoId(setup.photo_id), "e2e comment").await
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
            && row.photo_id == PhotoId(setup.photo_id)
            && row.content == "e2e comment";
        let photo_ok = photo_entity::Entity::find_by_id(PhotoId(setup.photo_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some_and(|p| p.comment_count > 0);
        Ok(row_ok && photo_ok)
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
                &format!("/photo/comment/{}", setup.photo_id),
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

/// 评论列表前置:登录 + 目标照片 + 前置评论。
#[derive(Default)]
pub struct CommentListSetup {
    pub session: Session,
    pub photo_id: i64,
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
            PhotoId(base.photo_id),
            "e2e list comment",
        )
        .await?
        .data;
        Ok(CommentListSetup {
            session: base.session,
            photo_id: base.photo_id,
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
                &format!("/photo/comment/{}", setup.photo_id),
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

/// 删除评论前置:登录 + 目标照片 + 前置评论。
#[derive(Default)]
pub struct CommentDeleteSetup {
    pub session: Session,
    pub photo_id: i64,
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
            PhotoId(base.photo_id),
            "e2e delete comment",
        )
        .await?
        .data;
        Ok(CommentDeleteSetup {
            session: base.session,
            photo_id: base.photo_id,
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
                &format!("/photo/comment/{}/{}", setup.photo_id, setup.comment_id),
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
