//! 并发正确性体检:同一资源上的竞态场景。
//!
//! 与常规场景的区别:
//! - 多个任务操作**同一份**数据(同一内容 / 同一月份 / 同一评论 / 同一收藏夹);
//! - `run` 前用共享 `Barrier` 对齐起跑, 把并发窗口压到最小;
//! - 结束后由 `reconcile` 直查数据库校验聚合不变量, 失败单独计入
//!   `invariant_failures`, 不混入单轮通过率。
//!
//! 覆盖的问题:
//! - 上传哈希去重的 TOCTOU(`exists_by_hash` 与 `INSERT` 之间无锁);
//! - 时间线统计「先 UPDATE 再 INSERT」在新月份首次并发上传时撞主键;
//! - 评论点赞计数与明细行漂移;
//! - 收藏夹 `visual_count` 与关联行漂移。

use std::time::Duration;

use chrono::Months;
use common_core::time::{DateTime, now};
use memory_seek_api::{
    ApiError, CollectionVisualAddBatchParam, CollectionVisualRemoveBatchParam, CommentContent,
    CommentPublishParam, VisualIds, api,
};
use memseek_test::{
    InvariantViolation, TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode, TeardownMode},
};
use sea_orm::{ConnectionTrait, Statement};
use types_visual::collection::CollectionId;
use types_visual::comment::CommentId;
use types_visual::dto::visual::VisualView;
use types_visual::visual::VisualId;

use crate::context::Context;
use crate::error::E2eError;
use crate::shared::RACE_START_TIMEOUT;

use super::{Session, blake3_hex, login, seed_visual, session, unique_png, unique_tag};

// ---------------------------------------------------------------------------
// 1. 同内容并发上传
// ---------------------------------------------------------------------------

/// 并发阶段共享的 PNG 内容 key(所有任务上传同一份字节)。
const SHARED_DUPLICATE_PNG: &str = "concurrent_duplicate_png";

/// 一次上传的结果。400(命中哈希去重)与 200(抢到插入)都是合法结果。
#[derive(Debug)]
pub enum DuplicateUploadOutcome {
    Created(VisualView),
    Duplicate,
}

/// 同内容并发上传:期望「一个成功 + 其余 400」, 不应出现 5xx。
///
/// 服务端先去重查询再插入, 两步之间没有锁也没有 `ON CONFLICT`;
/// 并发同内容时会撞 `visual_visual.hash` 唯一约束。按当前错误映射,
/// 后插入者可能变成 500 而不是业务上的 400。
#[derive(Default)]
pub struct ConcurrentDuplicateUploadScenario;

impl Scenario for ConcurrentDuplicateUploadScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = DuplicateUploadOutcome;
    type Setup = Session;

    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(session(ctx, task.index).await?)
    }

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 整轮共用同一份内容:第一次 run 初始化, 其它任务读到同一个 `Arc`。
        let bytes = ctx
            .shared
            .get_or_init(SHARED_DUPLICATE_PNG, || async {
                unique_png(&format!("dup-shared-{}", unique_tag(task)))
            })
            .await;
        ctx.shared
            .sync_start("dup_upload", task.total, RACE_START_TIMEOUT)
            .await;
        upload_shared(setup, bytes.as_slice()).await
    }

    /// 两个结果都算「业务允许」;真正的异常(5xx/传输错误)已经作为 `Err` 上抛。
    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    /// 收尾:抢到插入的那一轮由它自己删除, 保证下一轮仍是「首次上传」。
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let Some(Ok(DuplicateUploadOutcome::Created(view))) = result else {
            return Ok(());
        };
        crate::cleanup::idempotent(
            api::visual::delete_visuals(
                setup.client(),
                VisualIds::new(vec![view.id]).expect("单元素影像列表恒合法"),
            )
            .await,
        )
    }

    /// 对账:该内容不应留下任何影像行(收尾应已全部删除)。
    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let Some(bytes) = ctx.shared.peek::<Vec<u8>>(SHARED_DUPLICATE_PNG) else {
            return Ok(Vec::new());
        };
        let hash = blake3_hex(bytes.as_slice());
        let count = query_count(
            ctx,
            "SELECT count(*) AS n FROM visual_visual WHERE hash = $1",
            vec![hash.clone().into()],
        )
        .await?;

        let mut violations = Vec::new();
        if count != 0 {
            violations.push(InvariantViolation::new(
                "duplicate_upload_leftover",
                format!("hash={hash} 结算后仍有 {count} 行(收尾未清理干净)"),
            ));
        }
        Ok(violations)
    }
}

register_scenario!(ConcurrentDuplicateUploadScenario);

/// 上传共享内容:503(过载)有限重试, 400 归为「已存在」, 其它错误上抛。
async fn upload_shared(
    session: &Session,
    bytes: &[u8],
) -> Result<DuplicateUploadOutcome, E2eError> {
    const MAX_ATTEMPTS: u32 = 8;
    let mut backoff = Duration::from_millis(50);

    for attempt in 1..=MAX_ATTEMPTS {
        let result = api::visual::upload_visual(
            session.client(),
            bytes.to_vec(),
            Some("image/png".to_owned()),
            None,
        )
        .await;
        match result {
            Ok(view) => return Ok(DuplicateUploadOutcome::Created(view)),
            Err(ApiError::HttpStatus { code: 400, .. }) => {
                return Ok(DuplicateUploadOutcome::Duplicate);
            }
            Err(ApiError::HttpStatus { code: 503, .. }) if attempt < MAX_ATTEMPTS => {
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_millis(400));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(E2eError::internal("上传重试次数耗尽"))
}

// ---------------------------------------------------------------------------
// 2. 跨月首次并发上传
// ---------------------------------------------------------------------------

const CROSS_MONTH_MONTHS: &str = "cross_month_months";

/// 每次运行使用不同月份:回退月份数超过当前轮次, 保证每轮都是该月第一次上传。
#[derive(Default)]
pub struct CrossMonthFirstUploadScenario;

impl Scenario for CrossMonthFirstUploadScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = VisualView;
    type Setup = CrossMonthSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let retract = u32::try_from(task.round)
            .ok()
            .and_then(|round| round.checked_add(1))
            .ok_or_else(|| E2eError::internal("轮次超出月份可回退范围"))?;
        let created_at = now()
            .checked_sub_months(Months::new(retract))
            .ok_or_else(|| E2eError::internal("回退月份超出 chrono 范围"))?;
        // 指定 created_at 需要管理员身份; 同一账号并发登录会互相踢掉 token,
        // 因此整轮共享一份管理员会话。
        let session = ctx
            .shared
            .get_or_init("cross_month_admin_session", || async {
                login(ctx, "admin").await.map_err(|e| e.to_string())
            })
            .await;
        let session = (*session).clone().map_err(E2eError::internal)?;

        // 该月份必须是「空月份」: 删除可能残留的统计行, 让本轮必然走 INSERT 分支。
        let month = month_key(created_at);
        ctx.db
            .execute_raw(Statement::from_sql_and_values(
                ctx.db.get_database_backend(),
                "DELETE FROM visual_timeline_stat WHERE date_str = $1",
                [month.clone().into()],
            ))
            .await
            .map_err(|error| E2eError::internal(format!("清理目标月份统计失败: {error}")))?;
        ctx.shared.push(CROSS_MONTH_MONTHS, month);

        Ok(CrossMonthSetup {
            session,
            bytes: unique_png(&unique_tag(task)),
            created_at,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.shared
            .sync_start("cross_month", task.total, RACE_START_TIMEOUT)
            .await;
        upload_at(setup).await
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(month_key(output.created_at) == month_key(setup.created_at))
    }

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let Some(Ok(view)) = result else {
            return Ok(());
        };
        crate::cleanup::idempotent(
            api::visual::delete_visuals(
                setup.session.client(),
                VisualIds::new(vec![view.id]).expect("单元素影像列表恒合法"),
            )
            .await,
        )
    }

    /// 对账:本轮触碰过的月份, 统计行必须与影像实际数量一致; 空月份不应有统计行。
    ///
    /// 并发删除时 `refresh_after_delete` 各自按「剩余影像」重算, 互不可见对方的
    /// 未提交删除, 容易留下「已无影像但 count 仍 > 0」的陈旧行。
    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let mut months = ctx.shared.take(CROSS_MONTH_MONTHS);
        months.sort();
        months.dedup();

        let mut violations = Vec::new();
        for month in months {
            let actual = query_count(
                ctx,
                "SELECT count(*) AS n FROM visual_visual \
                 WHERE to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM') = $1",
                vec![month.clone().into()],
            )
            .await?;
            let stat = query_count(
                ctx,
                "SELECT COALESCE(max(count), 0) AS n FROM visual_timeline_stat WHERE date_str = $1",
                vec![month.clone().into()],
            )
            .await?;
            if actual != stat {
                violations.push(InvariantViolation::new(
                    "timeline_stat_mismatch",
                    format!("month={month} 实际影像 {actual} 条, 统计行 {stat} 条"),
                ));
            }
        }
        Ok(violations)
    }
}

register_scenario!(CrossMonthFirstUploadScenario);

pub struct CrossMonthSetup {
    pub session: Session,
    pub bytes: Vec<u8>,
    pub created_at: DateTime,
}

async fn upload_at(setup: &CrossMonthSetup) -> Result<VisualView, E2eError> {
    const MAX_ATTEMPTS: u32 = 8;
    let mut backoff = Duration::from_millis(50);
    for attempt in 1..=MAX_ATTEMPTS {
        let result = api::visual::upload_visual(
            setup.session.client(),
            setup.bytes.clone(),
            Some("image/png".to_owned()),
            Some(setup.created_at),
        )
        .await;
        match result {
            Ok(view) => return Ok(view),
            Err(ApiError::HttpStatus { code: 503, .. }) if attempt < MAX_ATTEMPTS => {
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_millis(400));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(E2eError::internal("上传重试次数耗尽"))
}

fn month_key(at: DateTime) -> String {
    at.format("%Y-%m").to_string()
}

// ---------------------------------------------------------------------------
// 3. 同一评论并发点赞
// ---------------------------------------------------------------------------

/// 热点评论所在影像的种子序号(区别于 like/comment 现有场景)。
const HOT_COMMENT_VISUAL_ORDINAL: u64 = 10;
const HOT_COMMENT_ID: &str = "hot_comment_id";

/// 共享热点评论场景中的任务角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotCommentRole {
    /// 独立账号,循环 like → unlike。
    Writer,
    /// 共享同一账号,并发 like/unlike 同一条评论。
    Racer,
}

fn hot_comment_role(task: &TaskIndex) -> HotCommentRole {
    let racers = (task.total / 8).max(2);
    if task.index < racers {
        HotCommentRole::Racer
    } else {
        HotCommentRole::Writer
    }
}

pub enum HotCommentOutput {
    Writer(u16),
    Racer(u16),
}

pub struct HotCommentSetup {
    pub session: Session,
    pub comment_id: CommentId,
    pub role: HotCommentRole,
}

/// 同一评论上的高并发点赞/取消,结束后对账计数与明细行数。
#[derive(Default)]
pub struct HotCommentLikeConcurrencyScenario;

impl Scenario for HotCommentLikeConcurrencyScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = HotCommentOutput;
    type Setup = HotCommentSetup;

    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        if task.total < 4 {
            return Err(E2eError::internal(
                "HotCommentLikeConcurrencyScenario 并发度需 >= 4",
            ));
        }
        let role = hot_comment_role(task);
        let session = match role {
            HotCommentRole::Racer => {
                let session = ctx
                    .shared
                    .get_or_init("hot_comment_racer_session", || async {
                        login(ctx, "e2e_shared_1").await.map_err(|e| e.to_string())
                    })
                    .await;
                (*session).clone().map_err(E2eError::internal)?
            }
            HotCommentRole::Writer => session(ctx, task.index).await?,
        };

        // 评论只在首次并发前创建一条;各场景复用同一条用于对抗点赞。
        let comment = ctx
            .shared
            .get_or_init(HOT_COMMENT_ID, || async {
                create_hot_comment(ctx, &session)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await;
        let comment_id = (*comment)
            .clone()
            .map_err(|error| E2eError::internal(format!("创建热点评论失败: {error}")))?;

        if role == HotCommentRole::Racer {
            let _ = api::visual::comment::unlike_comment(session.client(), comment_id).await;
        }

        Ok(HotCommentSetup {
            session,
            comment_id,
            role,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let result =
            api::visual::comment::like_comment(setup.session.client(), setup.comment_id).await;
        let code = crate::error::status(result)?;
        Ok(match setup.role {
            HotCommentRole::Writer => HotCommentOutput::Writer(code),
            HotCommentRole::Racer => HotCommentOutput::Racer(code),
        })
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(match (setup.role, output) {
            (HotCommentRole::Writer, HotCommentOutput::Writer(code)) => *code == 200,
            (HotCommentRole::Racer, HotCommentOutput::Racer(code)) => *code == 200 || *code == 400,
            _ => false,
        })
    }

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        match api::visual::comment::unlike_comment(setup.session.client(), setup.comment_id).await {
            Ok(_) => Ok(()),
            Err(ApiError::HttpStatus {
                code: 400 | 404, ..
            }) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let Some(comment) = ctx.shared.peek::<Result<CommentId, String>>(HOT_COMMENT_ID) else {
            return Ok(Vec::new());
        };
        let Ok(comment_id) = comment.as_ref() else {
            return Ok(vec![InvariantViolation::new(
                "hot_comment_missing",
                "热点评论未创建成功,无法对账",
            )]);
        };

        let row = ctx
            .db
            .query_one_raw(Statement::from_sql_and_values(
                ctx.db.get_database_backend(),
                "SELECT like_count, \
                        (SELECT count(*) FROM visual_comment_like WHERE comment_id = $1) AS actual \
                 FROM visual_comment WHERE id = $1",
                [comment_id.0.into()],
            ))
            .await
            .map_err(|error| E2eError::internal(format!("评论对账查询失败: {error}")))?
            .ok_or_else(|| E2eError::internal("评论在结算时不存在"))?;
        let expected: i32 = row
            .try_get("", "like_count")
            .map_err(|error| E2eError::internal(format!("读取 like_count 失败: {error}")))?;
        let expected = i64::from(expected);
        let actual: i64 = row
            .try_get("", "actual")
            .map_err(|error| E2eError::internal(format!("读取点赞行数失败: {error}")))?;

        let mut violations = Vec::new();
        if expected != actual {
            violations.push(InvariantViolation::new(
                "comment_like_count_mismatch",
                format!(
                    "comment={} expected={expected} actual={actual}",
                    comment_id.0
                ),
            ));
        }
        if expected != 0 {
            violations.push(InvariantViolation::new(
                "comment_like_not_settled",
                format!("comment={} 结算后仍为 {expected}", comment_id.0),
            ));
        }

        let duplicates = query_count(
            ctx,
            "SELECT count(*) AS n FROM ( \
                 SELECT user_id FROM visual_comment_like WHERE comment_id = $1 \
                 GROUP BY user_id HAVING count(*) > 1 \
             ) duplicated",
            vec![comment_id.0.into()],
        )
        .await?;
        if duplicates > 0 {
            violations.push(InvariantViolation::new(
                "duplicate_comment_like_rows",
                format!("comment={} 存在 {duplicates} 个重复点赞组", comment_id.0),
            ));
        }
        Ok(violations)
    }
}

register_scenario!(HotCommentLikeConcurrencyScenario);

async fn create_hot_comment(ctx: &Context, session: &Session) -> Result<CommentId, E2eError> {
    let visual = seed_visual(ctx, HOT_COMMENT_VISUAL_ORDINAL)
        .await
        .ok_or_else(super::seed_missing)?;
    let param = CommentPublishParam {
        content: CommentContent::new("hot-comment-concurrency".to_owned())
            .expect("e2e 评论内容非空且不超限"),
    };
    let view = api::visual::comment::publish_comment(session.client(), visual.id, &param).await?;
    Ok(view.id)
}

// ---------------------------------------------------------------------------
// 4. 同一收藏夹并发添加
// ---------------------------------------------------------------------------

const HOT_COLLECTION_ID: &str = "hot_collection_id";

/// 收藏夹口径的并发场景:所有任务把同一批影像并发加入同一收藏夹。
#[derive(Default)]
pub struct HotCollectionConcurrencyScenario;

impl Scenario for HotCollectionConcurrencyScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = u64;
    type Setup = HotCollectionSetup;

    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = ctx
            .shared
            .get_or_init("hot_collection_session", || async {
                login(ctx, "e2e_shared_2").await.map_err(|e| e.to_string())
            })
            .await;
        let session = (*session).clone().map_err(E2eError::internal)?;

        let collection = ctx
            .shared
            .get_or_init(HOT_COLLECTION_ID, || async {
                create_hot_collection(&session)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await;
        let collection_id = (*collection)
            .clone()
            .map_err(|error| E2eError::internal(format!("创建热点收藏夹失败: {error}")))?;

        let mut visual_ids = Vec::new();
        for ordinal in 11..=14u64 {
            let visual = seed_visual(ctx, ordinal)
                .await
                .ok_or_else(super::seed_missing)?;
            visual_ids.push(visual.id);
        }

        Ok(HotCollectionSetup {
            session,
            collection_id,
            visual_ids,
            task_total: task.total,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.shared
            .sync_start("hot_collection", setup.task_total, RACE_START_TIMEOUT)
            .await;
        let param = CollectionVisualAddBatchParam {
            visual_ids: VisualIds::new(setup.visual_ids.clone()).expect("非空影像列表恒合法"),
        };
        let result = api::visual::collection::add_visuals_to_collection(
            setup.session.client(),
            setup.collection_id,
            &param,
        )
        .await?;
        Ok(result.new_visual_count)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 并发下每个请求新增数不确定, 但不得超过批次大小。
        Ok(*output <= setup.visual_ids.len() as u64)
    }

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let param = CollectionVisualRemoveBatchParam {
            visual_ids: VisualIds::new(setup.visual_ids.clone()).expect("非空影像列表恒合法"),
        };
        match api::visual::collection::remove_visuals_from_collection(
            setup.session.client(),
            setup.collection_id,
            &param,
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(ApiError::HttpStatus { code: 404, .. }) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let Some(collection) = ctx
            .shared
            .peek::<Result<CollectionId, String>>(HOT_COLLECTION_ID)
        else {
            return Ok(Vec::new());
        };
        let Ok(collection_id) = collection.as_ref() else {
            return Ok(vec![InvariantViolation::new(
                "hot_collection_missing",
                "热点收藏夹未创建成功,无法对账",
            )]);
        };

        let row = ctx
            .db
            .query_one_raw(Statement::from_sql_and_values(
                ctx.db.get_database_backend(),
                "SELECT visual_count, \
                        (SELECT count(*) FROM visual_collection_visual WHERE collection_id = $1) AS actual \
                 FROM visual_collection WHERE id = $1",
                [collection_id.0.into()],
            ))
            .await
            .map_err(|error| E2eError::internal(format!("收藏夹对账查询失败: {error}")))?
            .ok_or_else(|| E2eError::internal("收藏夹在结算时不存在"))?;
        let expected: i64 = row
            .try_get("", "visual_count")
            .map_err(|error| E2eError::internal(format!("读取 visual_count 失败: {error}")))?;
        let actual: i64 = row
            .try_get("", "actual")
            .map_err(|error| E2eError::internal(format!("读取关联行数失败: {error}")))?;

        let mut violations = Vec::new();
        if expected != actual {
            violations.push(InvariantViolation::new(
                "collection_visual_count_mismatch",
                format!(
                    "collection={} expected={expected} actual={actual}",
                    collection_id.0
                ),
            ));
        }
        if expected != 0 {
            violations.push(InvariantViolation::new(
                "collection_visual_count_not_settled",
                format!("collection={} 结算后仍为 {expected}", collection_id.0),
            ));
        }
        Ok(violations)
    }
}

register_scenario!(HotCollectionConcurrencyScenario);

pub struct HotCollectionSetup {
    pub session: Session,
    pub collection_id: CollectionId,
    pub visual_ids: Vec<VisualId>,
    pub task_total: usize,
}

async fn create_hot_collection(session: &Session) -> Result<CollectionId, E2eError> {
    let name = format!(
        "e2e_hot_collection_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    );
    let param = memory_seek_api::CollectionCreateParam {
        name,
        description: Some("e2e".to_owned()),
    };
    let view = api::visual::collection::create_collection(session.client(), &param).await?;
    Ok(view.id)
}

// ---------------------------------------------------------------------------
// 公共查询
// ---------------------------------------------------------------------------

async fn query_count(
    ctx: &Context,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> Result<i64, E2eError> {
    let stmt = Statement::from_sql_and_values(ctx.db.get_database_backend(), sql, values);
    let row = ctx
        .db
        .query_one_raw(stmt)
        .await
        .map_err(|error| E2eError::internal(format!("对账查询失败: {error}")))?
        .ok_or_else(|| E2eError::internal("对账查询无结果"))?;
    row.try_get("", "n")
        .map_err(|error| E2eError::internal(format!("读取计数失败: {error}")))
}
