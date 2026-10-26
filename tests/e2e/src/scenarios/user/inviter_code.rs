use memory_seek_api::api;
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, TeardownMode},
};
use types_identity::user::InviterCodeView;

use crate::context::Context;
use crate::error::E2eError;

use super::session::{Session, login, user_account};

/// 生成邀请码: 校验 Redis 已写入 `邀请码 -> user_id` 且带 TTL.
#[derive(Default)]
pub struct GenerateInviterCodeScenario;

impl Scenario for GenerateInviterCodeScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = InviterCodeView;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &user_account(task.index)).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::user::generate_inviter_code(setup.client()).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let key = constants::redis_keys::auth::inviter_code(&output.inviter_code);
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
        let key = constants::redis_keys::auth::inviter_code(&view.inviter_code);
        if !ctx.redis_del(&key).await {
            tracing::warn!(%key, "清理邀请码失败");
        }
        Ok(())
    }
}

register_scenario!(GenerateInviterCodeScenario);
