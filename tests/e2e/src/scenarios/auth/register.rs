use common::{
    axum::{ErrR, SucR},
    ext::ToOk,
};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::HttpError,
    register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, SelectExt};
use serde_json::json;
use types::auth::{self};
use types::user::UserInfo;

use crate::context::Context;

/// 注册成功: Round 模式下每轮 run 前 setup 发送验证码并从 MailHog 读回,
/// 每轮使用全新的账号+验证码(验证码注册后即失效, 不可跨轮复用)。
#[derive(Default)]
pub struct RegisterSetup {
    pub username: String,
    pub email: String,
    pub code: String,
}

#[derive(Default)]
pub struct RegisterScenario;
impl Scenario for RegisterScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<UserInfo>;

    type Setup = RegisterSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    /// 预置: 发验证码 + 从 MailHog 读回真实验证码(独立命名空间避免读旧邮件)
    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let username = format!("e2e_reg_{}_{}", task.index, task.round);
        let email = format!("e2e_reg_{}_{}@test.com", task.index, task.round);

        ctx.client
            .post("/auth/verification-codes", json!({ "email": email }))
            .await?;
        let code = ctx.wait_mailhog_code(&email).await?;
        Ok(RegisterSetup {
            username,
            email,
            code,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .post(
                "/auth/register",
                json!({
                    "username": setup.username,
                    "email": setup.email,
                    "password": "Test123456",
                    "confirmPassword": "Test123456",
                    "nickname": "E2E",
                    "inviterCode": "DRIFTC",
                    "emailVerifyCode": setup.code,
                }),
            )
            .await?
            .json::<Self::Output>()
            .await?
            .to_ok()
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 回查: 新用户确实落库
        let exists = auth::user::Entity::find()
            .filter(auth::user::Column::Username.eq(&setup.username))
            .exists(&ctx.db)
            .await
            .unwrap();
        Ok(exists)
    }
}

register_scenario!(RegisterScenario, mode = memseek_test::RunMode::Times(32));

/// 注册失败 - 邮箱验证码错误: 期望 400。
#[derive(Default)]
pub struct RegisterInvalidCodeScenario;
impl Scenario for RegisterInvalidCodeScenario {
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
            .post_raw(
                "/auth/register",
                json!({
                    "username": format!("e2e_inv_{}", task.index),
                    "email": format!("e2e_inv_{}@test.com", task.index),
                    "password": "Test123456",
                    "confirmPassword": "Test123456",
                    "nickname": "E2E",
                    "inviterCode": "DRIFTC",
                    "emailVerifyCode": "000000",
                }),
            )
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
    RegisterInvalidCodeScenario,
    mode = memseek_test::RunMode::Times(32)
);
