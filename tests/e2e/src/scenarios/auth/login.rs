use memory_seek_api::api;
use memseek_test::{TaskIndex, register_scenario, scenario::Scenario};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, SelectExt};
use types_identity::auth::{self, LoginRequest, LoginResponse};

use crate::context::Context;
use crate::error::{E2eError, status};

/// 登录成功: 复用种子用户 `loadtest_{index+1}`, 校验响应 token 与库中一致。
#[derive(Default)]
pub struct LoginScenario;
impl Scenario for LoginScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = LoginResponse;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let request = LoginRequest {
            account: format!("loadtest_{}", task.index + 1),
            password: "Test123456".to_owned(),
        };
        Ok(api::auth::login(&ctx.api_client(), &request).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(output.user.id))
            .filter(auth::user::Column::RefreshToken.eq(&output.refresh_token))
            .exists(&ctx.db)
            .await
            .unwrap())
    }
}

register_scenario!(LoginScenario);

/// 登录失败 - 密码错误: 期望 400 业务错误。
#[derive(Default)]
pub struct LoginWrongPasswordScenario;
impl Scenario for LoginWrongPasswordScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let request = LoginRequest {
            account: "loadtest_1".to_owned(),
            password: "WrongPass1".to_owned(),
        };
        status(api::auth::login(&ctx.api_client(), &request).await)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 400)
    }
}

register_scenario!(LoginWrongPasswordScenario);

/// 登录失败 - 账号不存在: 期望 400(与密码错误响应一致, 防用户枚举)。
#[derive(Default)]
pub struct LoginUnknownAccountScenario;
impl Scenario for LoginUnknownAccountScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let request = LoginRequest {
            account: format!("nobody_{}", task.index),
            password: "Test123456".to_owned(),
        };
        status(api::auth::login(&ctx.api_client(), &request).await)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 400)
    }
}

register_scenario!(LoginUnknownAccountScenario);
