use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types::auth;
use types::user::UserInfo;

use crate::context::Context;

use super::session::{Session, login, user_account};

/// 获取当前用户信息: 响应必须与库中记录一致.
#[derive(Default)]
pub struct MeScenario;

impl Scenario for MeScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<UserInfo>;

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
            .request(reqwest::Method::GET, "/user/me")
            .header("Authorization", &setup.auth_header())
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
        let db_user = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.user_id))
            .one(&ctx.db)
            .await
            .unwrap();

        Ok(output.data.id == setup.user_id
            && db_user.is_some_and(|u| {
                u.username == output.data.username
                    && u.email == output.data.email
                    && u.nickname == output.data.nickname
            }))
    }
}

register_scenario!(MeScenario, mode = memseek_test::RunMode::Times(32));

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct MeUnauthorizedScenario;

impl Scenario for MeUnauthorizedScenario {
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
            .get_raw("/user/me")
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

register_scenario!(
    MeUnauthorizedScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 伪造 access_token(用户存在但 token 不匹配): 期望 401.
#[derive(Default)]
pub struct MeForgedTokenScenario;

impl Scenario for MeForgedTokenScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let authorization = format!("Bearer {} forged_token", task.index + 1);
        ctx.client
            .request(reqwest::Method::GET, "/user/me")
            .header("Authorization", &authorization)
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

register_scenario!(
    MeForgedTokenScenario,
    mode = memseek_test::RunMode::Times(32)
);
