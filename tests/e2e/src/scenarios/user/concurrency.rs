//! user 模块并发正确性体检:同账号写竞争。
//!
//! 与常规场景的「每任务独立账号」不同, 这里的多个任务**共用同一个账号**,
//! 用共享 `Barrier` 对齐起跑, 观察服务端是否存在丢失更新 / 悬挂引用:
//! - 并发换头像: 最终库中只能有一个 key, 且被替换掉的旧对象必须被删除;
//! - 并发改密: 同一个旧密码被并发消费时, 只应有一个请求成功。

use memory_seek_api::{ApiError, api};
use memseek_test::{InvariantViolation, TaskIndex, register_scenario, scenario::Scenario};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_identity::auth;
use types_identity::user::ChangePasswordParam;

use crate::context::Context;
use crate::error::E2eError;
use crate::scenarios::visual::{unique_png, unique_tag};
use crate::shared::RACE_START_TIMEOUT;

use super::session::{PASSWORD, Session, login, login_with};

// ---------------------------------------------------------------------------
// 1. 同账号并发换头像
// ---------------------------------------------------------------------------

/// 并发换头像专用账号。
const AVATAR_ACCOUNT: &str = "e2e_shared_4";
const AVATAR_SESSION: &str = "shared_avatar_session";
const AVATAR_KEYS: &str = "shared_avatar_keys";

/// 同账号并发换头像:成功返回的 key 全部落盘, 最终只保留最后一次写入的对象。
#[derive(Default)]
pub struct SharedAvatarReplaceScenario;

impl Scenario for SharedAvatarReplaceScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = ();
    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, _task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = ctx
            .shared
            .get_or_init(AVATAR_SESSION, || async {
                login(ctx, AVATAR_ACCOUNT).await.map_err(|e| e.to_string())
            })
            .await;
        (*session).clone().map_err(E2eError::internal)
    }

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let bytes = unique_png(&unique_tag(task));
        ctx.shared
            .sync_start("shared_avatar", task.total, RACE_START_TIMEOUT)
            .await;
        let token =
            api::user::upload_avatar(setup.client(), bytes, Some("image/png".to_owned())).await?;
        let key = token
            .decrypt()
            .map_err(|error| E2eError::internal(format!("头像 token 解密失败: {error}")))?
            .file_id;
        ctx.shared.push(AVATAR_KEYS, key);
        Ok(())
    }

    /// 对账:最终 key 存在;所有被替换掉的 key 必须已从对象存储删除。
    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let Some(session) = ctx.shared.peek::<Session>(AVATAR_SESSION) else {
            return Ok(Vec::new());
        };
        let keys = ctx.shared.take(AVATAR_KEYS);
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let final_key = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(session.user_id))
            .one(&ctx.db)
            .await
            .map_err(|error| E2eError::internal(format!("查询用户头像失败: {error}")))?
            .and_then(|user| user.avatar_file_id);

        let mut violations = Vec::new();
        let Some(final_key) = final_key else {
            violations.push(InvariantViolation::new(
                "avatar_missing",
                "并发换头像后库中没有任何头像 key",
            ));
            return Ok(violations);
        };

        if !keys.iter().any(|key| key == &final_key) {
            violations.push(InvariantViolation::new(
                "avatar_final_key_unknown",
                format!("库中最终 key={final_key} 不在任何成功响应的 token 中"),
            ));
        }
        if ctx.s3_bytes(&final_key).await.is_none() {
            violations.push(InvariantViolation::new(
                "avatar_final_object_missing",
                format!("库中最终 key={final_key} 在对象存储中不存在"),
            ));
        }

        for key in keys.iter().filter(|key| **key != final_key) {
            if !ctx.s3_missing(key).await {
                violations.push(InvariantViolation::new(
                    "avatar_replaced_object_leaked",
                    format!("已被替换的 key={key} 仍存在于对象存储"),
                ));
            }
        }
        Ok(violations)
    }
}

register_scenario!(SharedAvatarReplaceScenario);

// ---------------------------------------------------------------------------
// 2. 同账号并发改密
// ---------------------------------------------------------------------------

/// 并发改密专用账号。
const PASSWORD_ACCOUNT: &str = "e2e_shared_3";
const PASSWORD_SESSION: &str = "shared_password_session";
const PASSWORD_SUCCESS: &str = "shared_password_success";
const PASSWORD_CANDIDATES: &str = "shared_password_candidates";

/// 同一个旧密码被多个并发请求消费:只允许一个成功。
pub enum PasswordRaceOutcome {
    /// 本轮参与并发(携带实际状态码)。
    Attempted(u16),
    /// 仅第 0 轮做对抗, 其余轮次空转。
    Skipped,
}

#[derive(Default)]
pub struct SharedPasswordChangeScenario;

impl Scenario for SharedPasswordChangeScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = PasswordRaceOutcome;
    type Setup = Session;

    /// 只在第 0 轮抢跑:改密成功会清除会话, 后续轮次无法再登录旧密码。
    async fn setup(ctx: &Self::Ctx, _task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = ctx
            .shared
            .get_or_init(PASSWORD_SESSION, || async {
                login(ctx, PASSWORD_ACCOUNT)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
        (*session).clone().map_err(E2eError::internal)
    }

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        if task.round > 0 {
            return Ok(PasswordRaceOutcome::Skipped);
        }
        // 每个任务提交不同的新密码, 便于判断「谁最后生效」。
        let new_password = format!("Racepass{}", 1000 + task.index);
        let param = ChangePasswordParam {
            old_password: PASSWORD.to_owned(),
            new_password: new_password.clone(),
            confirm_password: new_password.clone(),
        };

        ctx.shared
            .sync_start("shared_password", task.total, RACE_START_TIMEOUT)
            .await;
        let result = api::user::change_password(setup.client(), &param).await;
        let code = match result {
            Ok(()) => 200,
            Err(ApiError::HttpStatus { code, .. }) => code,
            Err(error) => return Err(error.into()),
        };
        if code == 200 {
            ctx.shared.incr(PASSWORD_SUCCESS);
            ctx.shared.push(PASSWORD_CANDIDATES, new_password);
        }
        Ok(PasswordRaceOutcome::Attempted(code))
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(match output {
            PasswordRaceOutcome::Skipped => true,
            // 抢跑前已被登出(401)或旧密码已失效(400)都算正常竞争结果。
            PasswordRaceOutcome::Attempted(code) => {
                matches!(code, 200 | 400 | 401)
            }
        })
    }

    /// 对账:只允许一个成功, 并把密码还原回固定值保证可重复运行。
    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let successes = ctx.shared.get(PASSWORD_SUCCESS);
        let candidates = ctx.shared.take(PASSWORD_CANDIDATES);
        let mut violations = Vec::new();

        if successes != 1 {
            violations.push(InvariantViolation::new(
                "password_change_lost_update",
                format!(
                    "同一个旧密码被并发消费, 成功 {successes} 次(期望 1 次); 提交的新密码 {}",
                    candidates.join(", ")
                ),
            ));
        }

        // 还原密码:用任一成功的新密码登录后改回 PASSWORD, 保证下次运行可重复。
        let mut restored = false;
        for candidate in &candidates {
            if let Ok(session) = login_with(ctx, PASSWORD_ACCOUNT, candidate).await {
                let param = ChangePasswordParam {
                    old_password: candidate.clone(),
                    new_password: PASSWORD.to_owned(),
                    confirm_password: PASSWORD.to_owned(),
                };
                if api::user::change_password(session.client(), &param)
                    .await
                    .is_ok()
                {
                    restored = true;
                    break;
                }
            }
        }
        if !restored {
            violations.push(InvariantViolation::new(
                "password_restore_failed",
                "并发改密后无法用任何成功的新密码还原, 后续运行会因旧密码失效而失败",
            ));
        }
        Ok(violations)
    }
}

register_scenario!(SharedPasswordChangeScenario);
