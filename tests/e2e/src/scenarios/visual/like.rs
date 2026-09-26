//! 点赞:`/visual/visual/{visual_id}/like` 与 `/visual/comment/{comment_id}/like`。
//!
//! 各场景使用不同的种子影像(file_id 里的 user ordinal 前缀不同), 避免同一
//! (user, visual) 在不同场景重复点赞触发 400。

use common_core::types::CursorPage;
use common_web::SucR;
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types_core::cursor::TimeIdCursor;
use types_visual::comment::CommentId;
use types_visual::comment_like as comment_like_entity;
use types_visual::dto::comment::CommentView;
use types_visual::dto::visual::VisualView;
use types_visual::visual::VisualId;
use types_visual::visual_like as visual_like_entity;

use crate::context::Context;

use super::{Session, seed_visual, session};

/// 在影像下发表评论(用于评论点赞场景), 返回评论 id(原始 i64)。
async fn publish_comment(
    ctx: &Context,
    session: &Session,
    visual_id: VisualId,
) -> Result<i64, HttpError> {
    let view: CommentView = ctx
        .client
        .request(
            reqwest::Method::POST,
            &format!("/visual/comment/{visual_id}"),
        )
        .header("Authorization", &session.auth_header())
        .json_unwrap(&json!({ "content": "e2e like comment" }))
        .send_checked()
        .await?
        .json::<SucR<CommentView>>()
        .await
        .map_err(HttpError::from)?
        .data;
    Ok(view.id.0)
}

/// 影像点赞前置。
#[derive(Default)]
pub struct LikeVisualSetup {
    pub session: Session,
    pub visual_id: i64,
}

/// 点赞影像: 点赞记录落库。
#[derive(Default)]
pub struct LikeVisualScenario;

impl Scenario for LikeVisualScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = LikeVisualSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 2).await.ok_or_else(super::seed_missing)?;
        // 种子影像跨轮复用: 先取消点赞, 确保本轮处于未点赞状态.
        let _ = ctx
            .client
            .request(
                reqwest::Method::DELETE,
                &format!("/visual/visual/{}/like", visual.id),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await;
        Ok(LikeVisualSetup {
            session,
            visual_id: visual.id.0,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/visual/visual/{}/like", setup.visual_id),
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
        let liked = visual_like_entity::Entity::find()
            .filter(visual_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(visual_like_entity::Column::VisualId.eq(VisualId(setup.visual_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some();
        Ok(liked)
    }
}

register_scenario!(LikeVisualScenario);

/// 取消点赞影像前置:登录 + 影像 + 已点赞。
#[derive(Default)]
pub struct UnlikeVisualSetup {
    pub session: Session,
    pub visual_id: i64,
}

/// 取消点赞影像: 点赞记录消失。
#[derive(Default)]
pub struct UnlikeVisualScenario;

impl Scenario for UnlikeVisualScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = UnlikeVisualSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 3).await.ok_or_else(super::seed_missing)?;
        // 先点赞, 供 run 取消
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/visual/visual/{}/like", visual.id),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await?;
        Ok(UnlikeVisualSetup {
            session,
            visual_id: visual.id.0,
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
                &format!("/visual/visual/{}/like", setup.visual_id),
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
        let gone = visual_like_entity::Entity::find()
            .filter(visual_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(visual_like_entity::Column::VisualId.eq(VisualId(setup.visual_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(UnlikeVisualScenario);

/// 点赞列表前置:登录 + 影像 + 已点赞。
#[derive(Default)]
pub struct LikedVisualsSetup {
    pub session: Session,
    pub visual_id: i64,
}

/// 点赞影像分页: 包含前置点赞的影像。
#[derive(Default)]
pub struct GetLikedVisualsScenario;

impl Scenario for GetLikedVisualsScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CursorPage<VisualView, TimeIdCursor<VisualId>>>;

    type Setup = LikedVisualsSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 4).await.ok_or_else(super::seed_missing)?;
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/visual/visual/{}/like", visual.id),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await?;
        Ok(LikedVisualsSetup {
            session,
            visual_id: visual.id.0,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::GET, "/visual/visual/liked")
            .header("Authorization", &setup.session.auth_header())
            .query("size", "10")
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
            .any(|p| p.id.0 == setup.visual_id))
    }
}

register_scenario!(GetLikedVisualsScenario);

/// 点赞评论前置:登录 + 目标评论(种子影像 u=7)。
#[derive(Default)]
pub struct LikeCommentSetup {
    pub session: Session,
    pub comment_id: i64,
}

/// 点赞评论: 点赞记录落库。
#[derive(Default)]
pub struct LikeCommentScenario;

impl Scenario for LikeCommentScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = LikeCommentSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 7).await.ok_or_else(super::seed_missing)?;
        let comment_id = publish_comment(ctx, &session, visual.id).await?;
        Ok(LikeCommentSetup {
            session,
            comment_id,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/visual/comment/{}/like", setup.comment_id),
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
        let liked = comment_like_entity::Entity::find()
            .filter(comment_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(comment_like_entity::Column::CommentId.eq(CommentId(setup.comment_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some();
        Ok(liked)
    }
}

register_scenario!(LikeCommentScenario);

/// 取消点赞评论前置:登录 + 已点赞目标评论(种子影像 u=8)。
#[derive(Default)]
pub struct UnlikeCommentSetup {
    pub session: Session,
    pub comment_id: i64,
}

/// 取消点赞评论: 点赞记录消失。
#[derive(Default)]
pub struct UnlikeCommentScenario;

impl Scenario for UnlikeCommentScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = UnlikeCommentSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 8).await.ok_or_else(super::seed_missing)?;
        let comment_id = publish_comment(ctx, &session, visual.id).await?;
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/visual/comment/{comment_id}/like"),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await?;
        Ok(UnlikeCommentSetup {
            session,
            comment_id,
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
                &format!("/visual/comment/{}/like", setup.comment_id),
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
        let gone = comment_like_entity::Entity::find()
            .filter(comment_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(comment_like_entity::Column::CommentId.eq(CommentId(setup.comment_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(UnlikeCommentScenario);
