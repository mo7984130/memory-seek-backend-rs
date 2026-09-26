use common_core::ext::ToOk;
use common_web::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, SelectExt};
use serde_json::json;
use types_identity::auth::{self, LoginResponse};

use crate::context::Context;

/// 登录成功: 复用种子用户 `loadtest_{index+1}`, 校验响应 token 与库中一致。
#[derive(Default)]
pub struct LoginScenario;
impl Scenario for LoginScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<LoginResponse>;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/auth/login")
            .json_unwrap(&json!({
                "account": format!("loadtest_{}", task.index + 1),
                "password": "Test123456"
            }))
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await?
            .to_ok()
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(output.data.user.id))
            .filter(auth::user::Column::RefreshToken.eq(&output.data.refresh_token))
            .exists(&ctx.db)
            .await
            .unwrap()
            .to_ok()
    }
}

register_scenario!(LoginScenario);

/// 登录失败 - 密码错误: 期望 400 业务错误。
#[derive(Default)]
pub struct LoginWrongPasswordScenario;
impl Scenario for LoginWrongPasswordScenario {
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
            .request(reqwest::Method::POST, "/auth/login")
            .json_unwrap(&json!({ "account": "loadtest_1", "password": "WrongPass1" }))
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

register_scenario!(LoginWrongPasswordScenario);

/// 登录失败 - 账号不存在: 期望 400(与密码错误响应一致, 防用户枚举)。
#[derive(Default)]
pub struct LoginUnknownAccountScenario;
impl Scenario for LoginUnknownAccountScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/auth/login")
            .json_unwrap(&json!({
                "account": format!("nobody_{}", task.index),
                "password": "Test123456"
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

register_scenario!(LoginUnknownAccountScenario);
