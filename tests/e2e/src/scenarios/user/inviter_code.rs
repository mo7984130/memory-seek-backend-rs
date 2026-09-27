use common_web::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::{Scenario, TeardownMode},
};
use serde_json::json;
use types_identity::user::InviterCodeView;

use crate::context::Context;

use super::session::{Session, login, user_account};

/// 生成邀请码: 校验 Redis 已写入 `邀请码 -> user_id` 且带 TTL.
#[derive(Default)]
pub struct GenerateInviterCodeScenario;

impl Scenario for GenerateInviterCodeScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<InviterCodeView>;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &user_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/user/inviter-code")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({}))
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let key = constants::redis_keys::auth::inviter_code(&output.data.inviter_code);
        let expected = setup.user_id.0.to_string();
        let stored = ctx.redis_get(&key).await;
        let ttl_ok = ctx.redis_ttl(&key).await.is_some_and(|ttl| ttl > 0);

        Ok(stored.as_deref() == Some(expected.as_str()) && ttl_ok)
    }

    /// 收尾: 删除本轮生成的邀请码 key(每轮一个新码, 不回收会在 Redis 里堆积)
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn teardown(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let Some(Ok(view)) = result else {
            return Ok(());
        };
        let key = constants::redis_keys::auth::inviter_code(&view.data.inviter_code);
        if !ctx.redis_del(&key).await {
            tracing::warn!(%key, "清理邀请码失败");
        }
        Ok(())
    }
}

register_scenario!(GenerateInviterCodeScenario);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct GenerateInviterCodeUnauthorizedScenario;

impl Scenario for GenerateInviterCodeUnauthorizedScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/user/inviter-code")
            .json_unwrap(&json!({}))
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
        Ok(output.code == 401)
    }
}

register_scenario!(GenerateInviterCodeUnauthorizedScenario);
