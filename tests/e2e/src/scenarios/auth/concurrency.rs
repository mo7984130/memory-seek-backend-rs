//! auth 模块并发正确性体检:同一账号的会话竞争。
//!
//! 两类问题:
//! - 同账号并发登录:Redis access_token 与 DB refresh_token 必须成对存在,
//!   且最终 access_token 仍能通过鉴权;
//! - 刷新与登出交错:登出清掉会话后, 并发的刷新不应再「复活」出一个
//!   没有 refresh_token 支撑的孤立 access_token。

use memory_seek_api::{Method, api};
use memseek_test::{InvariantViolation, TaskIndex, register_scenario, scenario::Scenario};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_identity::auth;
use types_identity::auth::LoginRequest;

use crate::context::Context;
use crate::error::{E2eError, status};
use crate::shared::RACE_START_TIMEOUT;

use crate::scenarios::user::session::PASSWORD;

// ---------------------------------------------------------------------------
// 1. 同账号并发登录
// ---------------------------------------------------------------------------

const LOGIN_ACCOUNT: &str = "e2e_shared_5";
const LOGIN_ACCESS_TOKENS: &str = "shared_login_access_tokens";
const LOGIN_REFRESH_TOKENS: &str = "shared_login_refresh_tokens";
/// 只在前若干轮做对抗, 避免把整个压测时长耗在 Argon2 校验上。
const LOGIN_ROUNDS: usize = 8;

pub enum LoginRaceOutcome {
    Skipped,
    LoggedIn,
}

/// 同一账号并发登录:最终 Redis token 与 DB refresh_token 必须来自某些成功的登录。
#[derive(Default)]
pub struct SharedAccountLoginScenario;

impl Scenario for SharedAccountLoginScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = LoginRaceOutcome;
    type Setup = ();

    async fn setup(_ctx: &Self::Ctx, _task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(())
    }

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        if task.round >= LOGIN_ROUNDS {
            return Ok(LoginRaceOutcome::Skipped);
        }
        let request = LoginRequest {
            account: LOGIN_ACCOUNT.to_owned(),
            password: PASSWORD.to_owned(),
        };
        ctx.shared
            .sync_start("shared_login", task.total, RACE_START_TIMEOUT)
            .await;
        let response = api::auth::login(&ctx.api_client(), &request).await?;
        ctx.shared.push(LOGIN_ACCESS_TOKENS, response.access_token);
        ctx.shared
            .push(LOGIN_REFRESH_TOKENS, response.refresh_token);
        Ok(LoginRaceOutcome::LoggedIn)
    }

    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let access_tokens = ctx.shared.take(LOGIN_ACCESS_TOKENS);
        let refresh_tokens = ctx.shared.take(LOGIN_REFRESH_TOKENS);
        if access_tokens.is_empty() {
            return Ok(Vec::new());
        }

        let user = auth::user::Entity::find()
            .filter(auth::user::Column::Username.eq(LOGIN_ACCOUNT))
            .one(&ctx.db)
            .await
            .map_err(|error| E2eError::internal(format!("查询并发登录账号失败: {error}")))?
            .ok_or_else(|| E2eError::internal("并发登录账号不存在"))?;

        let access_key = constants::redis_keys::auth::user_access_token(user.id);
        let redis_token = ctx.redis_get(&access_key).await;
        let db_refresh = user.refresh_token;
        let mut violations = Vec::new();

        match redis_token {
            None => violations.push(InvariantViolation::new(
                "login_access_token_lost",
                format!(
                    "user={} 并发登录成功后 Redis 中没有 access_token(最后一次登录的写回可能失败并被回滚)",
                    user.id.0
                ),
            )),
            Some(token) => {
                if !access_tokens.iter().any(|candidate| candidate == &token) {
                    violations.push(InvariantViolation::new(
                        "login_access_token_unknown",
                        format!("user={} Redis access_token 不属于任何成功响应", user.id.0),
                    ));
                }
                let response = ctx
                    .api_client()
                    .raw(Method::GET, "/user/me")
                    .header("Authorization", format!("Bearer {} {token}", user.id.0))
                    .send()
                    .await
                    .map_err(|error| E2eError::internal(format!("鉴权探测失败: {error}")))?;
                if response.status.as_u16() != 200 {
                    violations.push(InvariantViolation::new(
                        "login_access_token_unusable",
                        format!(
                            "user={} Redis access_token 无法通过鉴权: {}",
                            user.id.0, response.status
                        ),
                    ));
                }
            }
        }

        match db_refresh {
            None => violations.push(InvariantViolation::new(
                "login_refresh_token_lost",
                format!("user={} 并发登录成功后库中没有 refresh_token", user.id.0),
            )),
            Some(token) => {
                if !refresh_tokens.iter().any(|candidate| candidate == &token) {
                    violations.push(InvariantViolation::new(
                        "login_refresh_token_unknown",
                        format!("user={} 库中 refresh_token 不属于任何成功响应", user.id.0),
                    ));
                }
            }
        }
        Ok(violations)
    }
}

register_scenario!(SharedAccountLoginScenario);

// ---------------------------------------------------------------------------
// 2. 刷新与登出交错
// ---------------------------------------------------------------------------

const SESSION_ACCOUNT: &str = "e2e_shared_6";
const SESSION_ROUNDS: usize = 16;
const ORPHAN_TOKENS: &str = "logout_refresh_orphans";

pub enum SessionRaceOutcome {
    Skipped,
    Probed { refresh: u16, logout: u16 },
}

/// 刷新与登出并发:不允许出现「Redis 有 access_token 但库里没有 refresh_token」。
#[derive(Default)]
pub struct LogoutRefreshRaceScenario;

impl Scenario for LogoutRefreshRaceScenario {
    type Ctx = Context;
    type Error = E2eError;
    type Output = SessionRaceOutcome;
    type Setup = ();

    async fn setup(_ctx: &Self::Ctx, _task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(())
    }

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 单任务内用两条独立客户端并发发请求, 保证两个请求同时在路上。
        if task.index != 0 || task.round >= SESSION_ROUNDS {
            return Ok(SessionRaceOutcome::Skipped);
        }

        let request = LoginRequest {
            account: SESSION_ACCOUNT.to_owned(),
            password: PASSWORD.to_owned(),
        };
        let login = api::auth::login(&ctx.api_client(), &request).await?;
        let user_id = login.user.id;

        let refresh_client = ctx.api_client();
        let logout_client = ctx.api_client();
        refresh_client
            .auth_storage()
            .set_login_result(&login)
            .expect("内存存储写入不应失败");
        logout_client
            .auth_storage()
            .set_login_result(&login)
            .expect("内存存储写入不应失败");

        let (refresh, logout) = tokio::join!(
            api::auth::refresh_token(&refresh_client, user_id, &login.refresh_token),
            api::user::logout(&logout_client),
        );
        let refresh_code = status(refresh)?;
        let logout_code = status(logout)?;

        // 结算:Redis access_token 存在但库中 refresh_token 为空 = 孤立 token。
        let access_key = constants::redis_keys::auth::user_access_token(user_id);
        let redis_token = ctx.redis_get(&access_key).await;
        let db_refresh = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(user_id))
            .one(&ctx.db)
            .await
            .map_err(|error| E2eError::internal(format!("查询会话失败: {error}")))?
            .and_then(|user| user.refresh_token);
        if redis_token.is_some() && db_refresh.is_none() {
            ctx.shared.incr(ORPHAN_TOKENS);
        }

        Ok(SessionRaceOutcome::Probed {
            refresh: refresh_code,
            logout: logout_code,
        })
    }

    async fn reconcile(ctx: &Self::Ctx) -> Result<Vec<InvariantViolation>, Self::Error> {
        let orphans = ctx.shared.get(ORPHAN_TOKENS);
        if orphans == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![InvariantViolation::new(
            "logout_refresh_orphan",
            format!(
                "{orphans} 轮出现「登出后并发刷新复活 access_token」: Redis 有 token 但库中 refresh_token 已被清空"
            ),
        )])
    }
}

register_scenario!(LogoutRefreshRaceScenario);
