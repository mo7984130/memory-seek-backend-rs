//! 点赞:`/visual/visual/{visual_id}/like` 与 `/visual/comment/{comment_id}/like`。
//!
//! 各场景使用不同的种子影像(file_id 里的 user ordinal 前缀不同), 避免同一
//! (user, visual) 在不同场景重复点赞触发 400。

use common_core::types::CursorPage;
use memory_seek_api::{CommentContent, CommentPublishParam, api};
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode, TeardownMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_core::cursor::TimeIdCursor;
use types_visual::comment::CommentId;
use types_visual::comment_like as comment_like_entity;
use types_visual::dto::visual::VisualView;
use types_visual::visual::VisualId;
use types_visual::visual_like as visual_like_entity;

use crate::context::Context;
use crate::error::E2eError;

use super::{Session, seed_visual, session};

/// 在影像下发表评论(用于评论点赞场景), 返回评论 id。
async fn publish_comment(session: &Session, visual_id: VisualId) -> Result<CommentId, E2eError> {
    let param = CommentPublishParam {
        content: CommentContent::new("e2e like comment".to_owned())
            .expect("e2e 评论内容非空且不超限"),
    };
    let view = api::visual::comment::publish_comment(session.client(), visual_id, &param).await?;
    Ok(view.id)
}

/// 幂等删除评论(收尾复用)。
async fn delete_comment(
    session: &Session,
    visual_id: VisualId,
    comment_id: CommentId,
) -> Result<(), E2eError> {
    crate::cleanup::idempotent(
        api::visual::comment::delete_comment(session.client(), visual_id, comment_id).await,
    )
}

/// 幂等取消点赞影像(收尾复用)。
async fn unlike_visual(session: &Session, visual_id: VisualId) -> Result<(), E2eError> {
    crate::cleanup::idempotent(api::visual::like::unlike_visual(session.client(), visual_id).await)
}

/// 影像点赞前置。
pub struct LikeVisualSetup {
    pub session: Session,
    pub visual_id: VisualId,
}

impl Default for LikeVisualSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
        }
    }
}

/// 点赞影像: 点赞记录落库。
#[derive(Default)]
pub struct LikeVisualScenario;

impl Scenario for LikeVisualScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = LikeVisualSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 2).await.ok_or_else(super::seed_missing)?;
        // 种子影像跨轮复用: 先取消点赞, 确保本轮处于未点赞状态.
        let _ = api::visual::like::unlike_visual(session.client(), visual.id).await;
        Ok(LikeVisualSetup {
            session,
            visual_id: visual.id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::like::like_visual(setup.session.client(), setup.visual_id).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let liked = visual_like_entity::Entity::find()
            .filter(visual_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(visual_like_entity::Column::VisualId.eq(setup.visual_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some();
        Ok(liked)
    }

    /// 收尾: 取消本任务在种子影像上的点赞, 离开时不留状态
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        unlike_visual(&setup.session, setup.visual_id).await
    }
}

register_scenario!(LikeVisualScenario);

/// 取消点赞影像前置:登录 + 影像 + 已点赞。
pub struct UnlikeVisualSetup {
    pub session: Session,
    pub visual_id: VisualId,
}

impl Default for UnlikeVisualSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
        }
    }
}

/// 取消点赞影像: 点赞记录消失。
#[derive(Default)]
pub struct UnlikeVisualScenario;

impl Scenario for UnlikeVisualScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = UnlikeVisualSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 3).await.ok_or_else(super::seed_missing)?;
        // 先点赞, 供 run 取消
        api::visual::like::like_visual(session.client(), visual.id).await?;
        Ok(UnlikeVisualSetup {
            session,
            visual_id: visual.id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::like::unlike_visual(setup.session.client(), setup.visual_id).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let gone = visual_like_entity::Entity::find()
            .filter(visual_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(visual_like_entity::Column::VisualId.eq(setup.visual_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(UnlikeVisualScenario);

/// 点赞列表前置:登录 + 影像 + 已点赞。
pub struct LikedVisualsSetup {
    pub session: Session,
    pub visual_id: VisualId,
}

impl Default for LikedVisualsSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
        }
    }
}

/// 点赞影像分页: 包含前置点赞的影像。
#[derive(Default)]
pub struct GetLikedVisualsScenario;

impl Scenario for GetLikedVisualsScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = CursorPage<VisualView, TimeIdCursor<VisualId>>;

    type Setup = LikedVisualsSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 4).await.ok_or_else(super::seed_missing)?;
        api::visual::like::like_visual(session.client(), visual.id).await?;
        Ok(LikedVisualsSetup {
            session,
            visual_id: visual.id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::like::get_liked_visuals(setup.session.client(), None, Some(10)).await?)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.records.iter().any(|p| p.id == setup.visual_id))
    }

    /// 收尾: 取消本任务的点赞, 让种子影像回到初始状态
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        unlike_visual(&setup.session, setup.visual_id).await
    }
}

register_scenario!(GetLikedVisualsScenario);

/// 点赞评论前置:登录 + 目标评论(种子影像 u=7)。
pub struct LikeCommentSetup {
    pub session: Session,
    pub visual_id: VisualId,
    pub comment_id: CommentId,
}

impl Default for LikeCommentSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
            comment_id: CommentId(0),
        }
    }
}

/// 点赞评论: 点赞记录落库。
#[derive(Default)]
pub struct LikeCommentScenario;

impl Scenario for LikeCommentScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = LikeCommentSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 7).await.ok_or_else(super::seed_missing)?;
        let comment_id = publish_comment(&session, visual.id).await?;
        Ok(LikeCommentSetup {
            session,
            visual_id: visual.id,
            comment_id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::comment::like_comment(setup.session.client(), setup.comment_id).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let liked = comment_like_entity::Entity::find()
            .filter(comment_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(comment_like_entity::Column::CommentId.eq(setup.comment_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some();
        Ok(liked)
    }

    /// 收尾: 删除 setup 阶段为点赞而发表的评论(服务端会一并清理其点赞)
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        delete_comment(&setup.session, setup.visual_id, setup.comment_id).await
    }
}

register_scenario!(LikeCommentScenario);

/// 取消点赞评论前置:登录 + 已点赞目标评论(种子影像 u=8)。
pub struct UnlikeCommentSetup {
    pub session: Session,
    pub visual_id: VisualId,
    pub comment_id: CommentId,
}

impl Default for UnlikeCommentSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
            comment_id: CommentId(0),
        }
    }
}

/// 取消点赞评论: 点赞记录消失。
#[derive(Default)]
pub struct UnlikeCommentScenario;

impl Scenario for UnlikeCommentScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = UnlikeCommentSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 8).await.ok_or_else(super::seed_missing)?;
        let comment_id = publish_comment(&session, visual.id).await?;
        api::visual::comment::like_comment(session.client(), comment_id).await?;
        Ok(UnlikeCommentSetup {
            session,
            visual_id: visual.id,
            comment_id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::comment::unlike_comment(setup.session.client(), setup.comment_id).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let gone = comment_like_entity::Entity::find()
            .filter(comment_like_entity::Column::UserId.eq(setup.session.user_id))
            .filter(comment_like_entity::Column::CommentId.eq(setup.comment_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }

    /// 收尾: 删除 setup 阶段为取消点赞而发表的评论
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        delete_comment(&setup.session, setup.visual_id, setup.comment_id).await
    }
}

register_scenario!(UnlikeCommentScenario);
