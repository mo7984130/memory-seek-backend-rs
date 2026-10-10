//! 评论:`/visual/comment`。

use common_core::types::CursorPage;
use memory_seek_api::{CommentContent, CommentPublishParam, Method, api};
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode, TeardownMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types_core::cursor::TimeIdCursor;
use types_visual::comment as comment_entity;
use types_visual::comment::CommentId;
use types_visual::dto::comment::CommentView;
use types_visual::visual as visual_entity;
use types_visual::visual::VisualId;

use crate::context::Context;
use crate::error::E2eError;

use super::{Session, seed_visual, session};

/// 在指定影像下发布评论, 返回视图。
async fn publish(
    session: &Session,
    visual_id: VisualId,
    content: &str,
) -> Result<CommentView, E2eError> {
    let param = CommentPublishParam {
        content: CommentContent::new(content.to_owned()).expect("e2e 评论内容非空且不超限"),
    };
    Ok(api::visual::comment::publish_comment(session.client(), visual_id, &param).await?)
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

/// 评论前置:登录 + 目标影像(种子影像 u=6)。
pub struct CommentSetup {
    pub session: Session,
    pub visual_id: VisualId,
}

impl Default for CommentSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
        }
    }
}

async fn comment_setup(ctx: &Context, task: &TaskIndex) -> Result<CommentSetup, E2eError> {
    comment_setup_on(ctx, task, 6).await
}

/// 指定种子影像序号的前置。
///
/// 各场景占用不同 ordinal 避免相互干扰: `GetComments` 断言"自己的评论在首页",
/// 若与持续发表评论的 `PublishComment` 共用同一张影像, 长跑下会被新评论挤出首页。
async fn comment_setup_on(
    ctx: &Context,
    task: &TaskIndex,
    ordinal: u64,
) -> Result<CommentSetup, E2eError> {
    let session = session(ctx, task.index).await?;
    let visual = seed_visual(ctx, ordinal)
        .await
        .ok_or_else(super::seed_missing)?;
    Ok(CommentSetup {
        session,
        visual_id: visual.id,
    })
}

/// 发布评论: 落库归属/内容正确, 影像评论数递增。
#[derive(Default)]
pub struct PublishCommentScenario;

impl Scenario for PublishCommentScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = CommentView;

    type Setup = CommentSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        comment_setup(ctx, task).await
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        publish(&setup.session, setup.visual_id, "e2e comment").await
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let Some(row) = comment_entity::Entity::find_by_id(output.id)
            .one(&ctx.db)
            .await
            .unwrap()
        else {
            return Ok(false);
        };
        let row_ok = row.user_id == setup.session.user_id
            && row.visual_id == setup.visual_id
            && row.content == "e2e comment";
        let visual_ok = visual_entity::Entity::find_by_id(setup.visual_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some_and(|p| p.comment_count > 0);
        Ok(row_ok && visual_ok)
    }

    /// 收尾: 删除本轮发表的评论(长跑下每轮新增一条)。
    /// 服务端删评论时会一并清理该评论的点赞与计数
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let Some(Ok(view)) = result else {
            return Ok(());
        };
        delete_comment(&setup.session, setup.visual_id, view.id).await
    }
}

register_scenario!(PublishCommentScenario);

/// 空内容评论: 期望 400(参数校验失败)。
#[derive(Default)]
pub struct PublishCommentEmptyScenario;

impl Scenario for PublishCommentEmptyScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = CommentSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        comment_setup(ctx, task).await
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 空内容无法构造(类型化 `CommentContent` 在客户端即拒绝), 故走 raw 直接发非法载荷
        let response = setup
            .session
            .client()
            .raw(Method::POST, format!("/visual/comment/{}", setup.visual_id))
            .json(&json!({ "content": "" }))
            .send()
            .await?;
        Ok(response.status.as_u16())
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 400)
    }
}

register_scenario!(PublishCommentEmptyScenario);

/// 评论列表前置:登录 + 目标影像 + 前置评论。
pub struct CommentListSetup {
    pub session: Session,
    pub visual_id: VisualId,
    pub comment_id: CommentId,
}

impl Default for CommentListSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
            comment_id: CommentId(0),
        }
    }
}

/// 评论游标列表: 包含前置发表的评论。
#[derive(Default)]
pub struct GetCommentsScenario;

impl Scenario for GetCommentsScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = CursorPage<CommentView, TimeIdCursor<CommentId>>;

    type Setup = CommentListSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        // 独占种子影像(ordinal 9): 本场景在 Task 粒度只建一条前置评论,
        // 与每轮都发表评论的 PublishComment 共用影像会让它被挤出首页
        let base = comment_setup_on(ctx, task, 9).await?;
        let comment = publish(&base.session, base.visual_id, "e2e list comment").await?;
        Ok(CommentListSetup {
            session: base.session,
            visual_id: base.visual_id,
            comment_id: comment.id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::comment::get_comment_list(
            setup.session.client(),
            setup.visual_id,
            None,
            Some(50),
        )
        .await?)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.records.iter().any(|c| c.id == setup.comment_id))
    }

    /// 收尾: 删除 setup 阶段制造的前置评论
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        delete_comment(&setup.session, setup.visual_id, setup.comment_id).await
    }
}

register_scenario!(GetCommentsScenario);

/// 删除评论前置:登录 + 目标影像 + 前置评论。
pub struct CommentDeleteSetup {
    pub session: Session,
    pub visual_id: VisualId,
    pub comment_id: CommentId,
}

impl Default for CommentDeleteSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
            comment_id: CommentId(0),
        }
    }
}

/// 删除评论: 评论记录消失。
#[derive(Default)]
pub struct DeleteCommentScenario;

impl Scenario for DeleteCommentScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = CommentDeleteSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let base = comment_setup(ctx, task).await?;
        let comment = publish(&base.session, base.visual_id, "e2e delete comment").await?;
        Ok(CommentDeleteSetup {
            session: base.session,
            visual_id: base.visual_id,
            comment_id: comment.id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::comment::delete_comment(
            setup.session.client(),
            setup.visual_id,
            setup.comment_id,
        )
        .await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let gone = comment_entity::Entity::find()
            .filter(comment_entity::Column::Id.eq(setup.comment_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(DeleteCommentScenario);
