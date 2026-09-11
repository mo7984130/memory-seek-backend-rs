use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types::auth;

use crate::context::Context;

use super::session::{
    NEW_PASSWORD, PASSWORD, Session, login, login_with, pwd_account, user_account,
};

/// 修改密码: 改密后服务端会登出 —— 旧 token 失效、refresh_token 清空、新密码可登录.
#[derive(Default)]
pub struct ChangePasswordScenario;

impl Scenario for ChangePasswordScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &pwd_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::PATCH, "/user/password")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({
                "oldPassword": PASSWORD,
                "newPassword": NEW_PASSWORD,
                "confirmPassword": NEW_PASSWORD,
            }))
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 1. 旧 access_token 已失效(改密成功后服务端调用了 logout)
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

        // 2. refresh_token 已清空
        let refresh_cleared = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.user_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some_and(|u| u.refresh_token.is_none());

        // 3. 新密码可重新登录
        let relogin = login_with(ctx, &pwd_account(task.index), NEW_PASSWORD)
            .await
            .is_ok();

        Ok(revoked && refresh_cleared && relogin)
    }
}

register_scenario!(
    ChangePasswordScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 旧密码错误: 期望 400.
#[derive(Default)]
pub struct ChangePasswordWrongOldScenario;

impl Scenario for ChangePasswordWrongOldScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

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
            .request(reqwest::Method::PATCH, "/user/password")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({
                "oldPassword": "WrongPass1",
                "newPassword": NEW_PASSWORD,
                "confirmPassword": NEW_PASSWORD,
            }))
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
        Ok(output.code == 400)
    }
}

register_scenario!(
    ChangePasswordWrongOldScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 两次输入不一致: 期望 400(请求体校验阶段拦截).
#[derive(Default)]
pub struct ChangePasswordMismatchScenario;

impl Scenario for ChangePasswordMismatchScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

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
            .request(reqwest::Method::PATCH, "/user/password")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({
                "oldPassword": PASSWORD,
                "newPassword": NEW_PASSWORD,
                "confirmPassword": "Different123",
            }))
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
        Ok(output.code == 400)
    }
}

register_scenario!(
    ChangePasswordMismatchScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct ChangePasswordUnauthorizedScenario;

impl Scenario for ChangePasswordUnauthorizedScenario {
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
            .request(reqwest::Method::PATCH, "/user/password")
            .json_unwrap(&json!({
                "oldPassword": PASSWORD,
                "newPassword": NEW_PASSWORD,
                "confirmPassword": NEW_PASSWORD,
            }))
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
    ChangePasswordUnauthorizedScenario,
    mode = memseek_test::RunMode::Times(32)
);
