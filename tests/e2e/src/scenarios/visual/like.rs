//! 点赞:`/visual/visual/{visual_id}/like` 与 `/visual/comment/{comment_id}/like`。
//!
//! 各场景使用不同的种子影像(file_id 里的 user ordinal 前缀不同), 避免同一
//! (user, visual) 在不同场景重复点赞触发 400。

use common_core::types::CursorPageWithTotal;
use memory_seek_api::{ApiError, CommentContent, CommentPublishParam, api};
use memseek_test::{
    InvariantViolation, TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode, TeardownMode},
};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Statement};
use types_core::cursor::TimeIdCursor;
use types_visual::comment::CommentId;
use types_visual::comment_like as comment_like_entity;
use types_visual::dto::visual::VisualView;
use types_visual::visual::VisualId;
use types_visual::visual_like as visual_like_entity;

use crate::context::Context;
use crate::error::{E2eError, status};

use super::{Session, login, seed_visual, session};

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

    type Output = CursorPageWithTotal<VisualView, TimeIdCursor<VisualId>>;

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
        Ok(output.page.records.iter().any(|p| p.id == setup.visual_id))
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

// ---------------------------------------------------------------------------
// 共享热点:同一影像上多用户并发点赞/取消 + 同账号唯一约束竞争 + 读缓存
// ---------------------------------------------------------------------------

/// 热点影像的种子序号(现有场景使用 1-8,9 预留给本场景)。
const HOTSPOT_ORDINAL: u64 = 9;

/// 共享热点场景中的任务角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotRole {
    /// 独立账号,循环 like → unlike,打同一行 like_count。
    Writer,
    /// 共享同一账号,并发 like/unlike 同一 `(user, visual)`。
    Racer,
    /// 独立账号,只读详情,验证读路径与缓存。
    Reader,
}

/// 按总并发度切分角色:racer 约 1/8,reader 约 1/4,其余为 writer。
fn hot_roles(total: usize) -> (usize, usize, usize) {
    let racers = (total / 8).max(2);
    let readers = (total / 4).max(1);
    let writers = total.saturating_sub(racers + readers);
    (writers, racers, readers)
}

fn hot_role(task: &TaskIndex) -> HotRole {
    let (writers, racers, _) = hot_roles(task.total);
    if task.index < racers {
        HotRole::Racer
    } else if task.index < racers + writers {
        HotRole::Writer
    } else {
        HotRole::Reader
    }
}

/// 热点场景每轮产出。
pub enum HotLikeOutput {
    Writer(u16),
    Racer(u16),
    Reader(VisualView),
}

pub struct HotLikeSetup {
    pub session: Session,
    pub visual_id: VisualId,
    pub role: HotRole,
}

/// 同一影像上的高并发点赞/取消,结束后对账计数与明细行数。
#[derive(Default)]
pub struct HotLikeConcurrencyScenario;

impl Scenario for HotLikeConcurrencyScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = HotLikeOutput;
    type Setup = HotLikeSetup;

    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        if task.total < 4 {
            return Err(E2eError::internal(
                "HotLikeConcurrencyScenario 并发度需 >= 4",
            ));
        }
        let role = hot_role(task);
        let visual = seed_visual(ctx, HOTSPOT_ORDINAL)
            .await
            .ok_or_else(super::seed_missing)?;

        let session = match role {
            HotRole::Racer => {
                // 共享账号只登录一次:4 个 racer 拿到同一份 token。
                let account = format!("loadtest_visual_{}", task.total + 1);
                let session = ctx
                    .shared
                    .get_or_init("hot_like_racer_session", || async {
                        login(ctx, &account)
                            .await
                            .map_err(|error| error.to_string())
                    })
                    .await;
                (*session).clone().map_err(E2eError::internal)?
            }
            HotRole::Writer | HotRole::Reader => session(ctx, task.index).await?,
        };

        if role == HotRole::Racer {
            // 共享账号可能残留上一轮/上一次运行的点赞,先清到未点赞状态。
            let _ = api::visual::like::unlike_visual(session.client(), visual.id).await;
        }

        Ok(HotLikeSetup {
            session,
            visual_id: visual.id,
            role,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(match setup.role {
            HotRole::Writer => HotLikeOutput::Writer(status(
                api::visual::like::like_visual(setup.session.client(), setup.visual_id).await,
            )?),
            HotRole::Racer => HotLikeOutput::Racer(status(
                api::visual::like::like_visual(setup.session.client(), setup.visual_id).await,
            )?),
            HotRole::Reader => HotLikeOutput::Reader(
                api::visual::get_visual_info(setup.session.client(), setup.visual_id).await?,
            ),
        })
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(match (setup.role, output) {
            (HotRole::Writer, HotLikeOutput::Writer(code)) => *code == 200,
            // 共享账号:同一时刻只有一个请求能插入成功,其余应是 400。
            (HotRole::Racer, HotLikeOutput::Racer(code)) => *code == 200 || *code == 400,
            (HotRole::Reader, HotLikeOutput::Reader(view)) => {
                view.id == setup.visual_id && !view.is_liked
            }
            _ => false,
        })
    }

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        if setup.role == HotRole::Reader {
            return Ok(());
        }
        match api::visual::like::unlike_visual(setup.session.client(), setup.visual_id).await {
            Ok(_) => Ok(()),
            // 本轮没点赞成功 / 已被同账号其它 racer 取消:都算收尾完成。
            Err(ApiError::HttpStatus {
                code: 400 | 404, ..
            }) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let Some(visual) = seed_visual(ctx, HOTSPOT_ORDINAL).await else {
            return Ok(vec![InvariantViolation::new(
                "hot_visual_missing",
                "seed_file_9_1 不存在,无法对账",
            )]);
        };

        let backend = ctx.db.get_database_backend();
        let stmt = Statement::from_sql_and_values(
            backend,
            "SELECT like_count, \
                    (SELECT count(*) FROM visual_visual_like WHERE visual_id = $1) AS actual \
             FROM visual_visual WHERE id = $1",
            [visual.id.0.into()],
        );
        let row = ctx
            .db
            .query_one_raw(stmt)
            .await
            .map_err(|error| E2eError::internal(format!("热点影像对账查询失败: {error}")))?
            .ok_or_else(|| E2eError::internal("热点影像在结算时不存在"))?;
        let expected: i64 = row
            .try_get("", "like_count")
            .map_err(|error| E2eError::internal(format!("读取 like_count 失败: {error}")))?;
        let actual: i64 = row
            .try_get("", "actual")
            .map_err(|error| E2eError::internal(format!("读取 like 行数失败: {error}")))?;

        let mut violations = Vec::new();
        if expected != actual {
            violations.push(InvariantViolation::new(
                "like_count_mismatch",
                format!("visual={} expected={expected} actual={actual}", visual.id.0),
            ));
        }
        if expected < 0 {
            violations.push(InvariantViolation::new(
                "like_count_negative",
                format!("visual={} like_count={expected}", visual.id.0),
            ));
        }
        if expected != 0 {
            violations.push(InvariantViolation::new(
                "like_count_not_settled",
                format!(
                    "visual={} 结算后仍为 {expected}(所有任务 teardown 应已取消点赞)",
                    visual.id.0
                ),
            ));
        }

        let dup_stmt = Statement::from_sql_and_values(
            backend,
            "SELECT count(*) AS duplicates FROM ( \
                 SELECT user_id FROM visual_visual_like WHERE visual_id = $1 \
                 GROUP BY user_id HAVING count(*) > 1 \
             ) duplicated",
            [visual.id.0.into()],
        );
        let dup_row = ctx
            .db
            .query_one_raw(dup_stmt)
            .await
            .map_err(|error| E2eError::internal(format!("重复点赞对账查询失败: {error}")))?
            .ok_or_else(|| E2eError::internal("重复点赞对账无结果"))?;
        let duplicates: i64 = dup_row
            .try_get("", "duplicates")
            .map_err(|error| E2eError::internal(format!("读取重复行数失败: {error}")))?;
        if duplicates > 0 {
            violations.push(InvariantViolation::new(
                "duplicate_like_rows",
                format!(
                    "visual={} 存在 {duplicates} 个 (user_id, visual_id) 重复组",
                    visual.id.0
                ),
            ));
        }

        Ok(violations)
    }
}

register_scenario!(HotLikeConcurrencyScenario);
