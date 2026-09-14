//! 点赞:`/photo/photos/{photo_id}/like` 与 `/photo/comment/{comment_id}/like`。
//!
//! 各场景使用不同的种子照片(file_id 里的 user ordinal 前缀不同), 避免同一
//! (user, photo) 在不同场景重复点赞触发 400。

use common::axum::SucR;
use common::types::CursorPage;
use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types::cursor::TimeIdCursor;
use types::photo::comment::CommentId;
use types::photo::comment_like as comment_like_entity;
use types::photo::dto::comment::CommentView;
use types::photo::dto::photo::PhotoView;
use types::photo::photo::PhotoId;
use types::photo::photo_like as photo_like_entity;

use crate::context::Context;

use super::{Session, seed_photo, session};

/// 在照片下发表评论(用于评论点赞场景), 返回评论 id(原始 i64)。
async fn publish_comment(
    ctx: &Context,
    session: &Session,
    photo_id: PhotoId,
) -> Result<i64, HttpError> {
    let view: CommentView = ctx
        .client
        .request(reqwest::Method::POST, &format!("/photo/comment/{photo_id}"))
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

/// 照片点赞前置。
#[derive(Default)]
pub struct LikePhotoSetup {
    pub session: Session,
    pub photo_id: i64,
}

/// 点赞照片: 点赞记录落库。
#[derive(Default)]
pub struct LikePhotoScenario;

impl Scenario for LikePhotoScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = LikePhotoSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let photo = seed_photo(ctx, 2).await.ok_or_else(super::seed_missing)?;
        Ok(LikePhotoSetup {
            session,
            photo_id: photo.id.0,
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
                &format!("/photo/photos/{}/like", setup.photo_id),
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
        let liked = photo_like_entity::Entity::find()
            .filter(photo_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(photo_like_entity::Column::PhotoId.eq(PhotoId(setup.photo_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some();
        Ok(liked)
    }
}

register_scenario!(LikePhotoScenario, mode = memseek_test::RunMode::Times(32));

/// 取消点赞照片前置:登录 + 照片 + 已点赞。
#[derive(Default)]
pub struct UnlikePhotoSetup {
    pub session: Session,
    pub photo_id: i64,
}

/// 取消点赞照片: 点赞记录消失。
#[derive(Default)]
pub struct UnlikePhotoScenario;

impl Scenario for UnlikePhotoScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = UnlikePhotoSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let photo = seed_photo(ctx, 3).await.ok_or_else(super::seed_missing)?;
        // 先点赞, 供 run 取消
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/photo/photos/{}/like", photo.id),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await?;
        Ok(UnlikePhotoSetup {
            session,
            photo_id: photo.id.0,
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
                &format!("/photo/photos/{}/like", setup.photo_id),
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
        let gone = photo_like_entity::Entity::find()
            .filter(photo_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(photo_like_entity::Column::PhotoId.eq(PhotoId(setup.photo_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(UnlikePhotoScenario, mode = memseek_test::RunMode::Times(32));

/// 点赞列表前置:登录 + 照片 + 已点赞。
#[derive(Default)]
pub struct LikedPhotosSetup {
    pub session: Session,
    pub photo_id: i64,
}

/// 点赞照片分页: 包含前置点赞的照片。
#[derive(Default)]
pub struct GetLikedPhotosScenario;

impl Scenario for GetLikedPhotosScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CursorPage<PhotoView, TimeIdCursor<PhotoId>>>;

    type Setup = LikedPhotosSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let photo = seed_photo(ctx, 4).await.ok_or_else(super::seed_missing)?;
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/photo/photos/{}/like", photo.id),
            )
            .header("Authorization", &session.auth_header())
            .send_checked()
            .await?;
        Ok(LikedPhotosSetup {
            session,
            photo_id: photo.id.0,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::GET, "/photo/photos/liked")
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
        Ok(output.data.records.iter().any(|p| p.id.0 == setup.photo_id))
    }
}

register_scenario!(
    GetLikedPhotosScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 点赞评论前置:登录 + 目标评论(种子照片 u=7)。
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

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let photo = seed_photo(ctx, 7).await.ok_or_else(super::seed_missing)?;
        let comment_id = publish_comment(ctx, &session, photo.id).await?;
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
                &format!("/photo/comment/{}/like", setup.comment_id),
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

register_scenario!(LikeCommentScenario, mode = memseek_test::RunMode::Times(32));

/// 取消点赞评论前置:登录 + 已点赞目标评论(种子照片 u=8)。
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

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let photo = seed_photo(ctx, 8).await.ok_or_else(super::seed_missing)?;
        let comment_id = publish_comment(ctx, &session, photo.id).await?;
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/photo/comment/{comment_id}/like"),
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
                &format!("/photo/comment/{}/like", setup.comment_id),
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

register_scenario!(
    UnlikeCommentScenario,
    mode = memseek_test::RunMode::Times(32)
);
