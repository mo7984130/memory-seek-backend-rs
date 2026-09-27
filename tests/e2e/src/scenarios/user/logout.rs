use common_web::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types_identity::auth;

use crate::context::Context;

use super::session::{Session, login, logout_account};

/// 登出: Redis access_token 删除、refresh_token 清空、旧 token 复用被拒.
#[derive(Default)]
pub struct LogoutScenario;

impl Scenario for LogoutScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = Session;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &logout_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/user/logout")
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

        // 3. 旧 access_token 复用被拒
        let after = ctx
            .client
            .request(reqwest::Method::GET, "/user/me")
            .header("Authorization", &setup.auth_header())
            .send()
            .await?
            .json::<ErrR>()
            .await
            .map_err(HttpError::from)?;
        let revoked = after.code == 401;

        Ok(token_gone && refresh_cleared && revoked)
    }
}

register_scenario!(LogoutScenario);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct LogoutUnauthorizedScenario;

impl Scenario for LogoutUnauthorizedScenario {
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
            .request(reqwest::Method::POST, "/user/logout")
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

register_scenario!(LogoutUnauthorizedScenario);
