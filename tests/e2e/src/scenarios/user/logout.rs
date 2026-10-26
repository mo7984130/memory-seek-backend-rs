use memory_seek_api::{Method, api};
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_identity::auth;

use crate::context::Context;
use crate::error::{E2eError, status};

use super::session::{Session, login, logout_account};

/// 登出: Redis access_token 删除、refresh_token 清空、旧 token 复用被拒.
#[derive(Default)]
pub struct LogoutScenario;

impl Scenario for LogoutScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = Session;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &logout_account(task.index)).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        api::user::logout(setup.client()).await?;
        Ok(())
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 1. Redis 中的 access_token 已删除
        let token_key = constants::redis_keys::auth::user_access_token(setup.user_id);
        let token_gone = ctx.redis_get(&token_key).await.is_none();

        // 2. 库中 refresh_token 已清空
        let refresh_cleared = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.user_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some_and(|u| u.refresh_token.is_none());

        // 3. 旧 access_token 复用被拒: 再读 /user/me 应为 401
        let after = status(api::user::get_me(setup.client()).await)?;
        let revoked = after == 401;

        Ok(token_gone && refresh_cleared && revoked)
    }
}

register_scenario!(LogoutScenario);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct LogoutUnauthorizedScenario;

impl Scenario for LogoutUnauthorizedScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let client = ctx.api_client();
        let response = client
            .raw(Method::POST, "/user/logout")
            .no_auth()
            .json(&serde_json::json!({}))
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
        Ok(*output == 401)
    }
}

register_scenario!(LogoutUnauthorizedScenario);
