//! 点赞:`/media/medias/{media_id}/like` 与 `/media/comment/{comment_id}/like`。
//!
//! 各场景使用不同的种子媒体(file_id 里的 user ordinal 前缀不同), 避免同一
//! (user, media) 在不同场景重复点赞触发 400。

use common::axum::SucR;
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
use types::media::comment::CommentId;
use types::media::comment_like as comment_like_entity;
use types::media::dto::comment::CommentView;
use types::media::dto::media::MediaView;
use types::media::media::MediaId;
use types::media::media_like as media_like_entity;

use crate::context::Context;

use super::{Session, seed_media, session};

/// 在媒体下发表评论(用于评论点赞场景), 返回评论 id(原始 i64)。
async fn publish_comment(
    ctx: &Context,
    session: &Session,
    media_id: MediaId,
) -> Result<i64, HttpError> {
    let view: CommentView = ctx
        .client
        .request(reqwest::Method::POST, &format!("/media/comment/{media_id}"))
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

/// 媒体点赞前置。
#[derive(Default)]
pub struct LikeMediaSetup {
    pub session: Session,
    pub media_id: i64,
}

/// 点赞媒体: 点赞记录落库。
#[derive(Default)]
pub struct LikeMediaScenario;

impl Scenario for LikeMediaScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = LikeMediaSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let media = seed_media(ctx, 2).await.ok_or_else(super::seed_missing)?;
        // 种子媒体跨轮复用: 先取消点赞, 确保本轮处于未点赞状态.
        let _ = ctx
            .client
            .request(
                reqwest::Method::DELETE,
                &format!("/media/medias/{}/like", media.id),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await;
        Ok(LikeMediaSetup {
            session,
            media_id: media.id.0,
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
                &format!("/media/medias/{}/like", setup.media_id),
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
        let liked = media_like_entity::Entity::find()
            .filter(media_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(media_like_entity::Column::MediaId.eq(MediaId(setup.media_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some();
        Ok(liked)
    }
}

register_scenario!(LikeMediaScenario);

/// 取消点赞媒体前置:登录 + 媒体 + 已点赞。
#[derive(Default)]
pub struct UnlikeMediaSetup {
    pub session: Session,
    pub media_id: i64,
}

/// 取消点赞媒体: 点赞记录消失。
#[derive(Default)]
pub struct UnlikeMediaScenario;

impl Scenario for UnlikeMediaScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = UnlikeMediaSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let media = seed_media(ctx, 3).await.ok_or_else(super::seed_missing)?;
        // 先点赞, 供 run 取消
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/media/medias/{}/like", media.id),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await?;
        Ok(UnlikeMediaSetup {
            session,
            media_id: media.id.0,
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
                &format!("/media/medias/{}/like", setup.media_id),
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
        let gone = media_like_entity::Entity::find()
            .filter(media_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(media_like_entity::Column::MediaId.eq(MediaId(setup.media_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(UnlikeMediaScenario);

/// 点赞列表前置:登录 + 媒体 + 已点赞。
#[derive(Default)]
pub struct LikedMediasSetup {
    pub session: Session,
    pub media_id: i64,
}

/// 点赞媒体分页: 包含前置点赞的媒体。
#[derive(Default)]
pub struct GetLikedMediasScenario;

impl Scenario for GetLikedMediasScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CursorPage<MediaView, TimeIdCursor<MediaId>>>;

    type Setup = LikedMediasSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let media = seed_media(ctx, 4).await.ok_or_else(super::seed_missing)?;
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/media/medias/{}/like", media.id),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await?;
        Ok(LikedMediasSetup {
            session,
            media_id: media.id.0,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::GET, "/media/medias/liked")
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
        Ok(output.data.records.iter().any(|p| p.id.0 == setup.media_id))
    }
}

register_scenario!(GetLikedMediasScenario);

/// 点赞评论前置:登录 + 目标评论(种子媒体 u=7)。
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
        let media = seed_media(ctx, 7).await.ok_or_else(super::seed_missing)?;
        let comment_id = publish_comment(ctx, &session, media.id).await?;
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
                &format!("/media/comment/{}/like", setup.comment_id),
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

/// 取消点赞评论前置:登录 + 已点赞目标评论(种子媒体 u=8)。
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
        let media = seed_media(ctx, 8).await.ok_or_else(super::seed_missing)?;
        let comment_id = publish_comment(ctx, &session, media.id).await?;
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/media/comment/{comment_id}/like"),
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
                &format!("/media/comment/{}/like", setup.comment_id),
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
