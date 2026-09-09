use common::{
    axum::{ErrR, SucR},
    ext::ToOk,
};
use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, SelectExt};
use serde_json::json;
use types::auth::{self};
use types::user::UserInfo;

use crate::context::Context;

/// 注册成功: preset 每任务发送验证码并读回, 每轮 run 只做注册。
/// 数据用 `e2e_reg_{index}` 唯一命名, Times 须 ≤ 并发度保证每任务只跑一次。
#[derive(Default)]
pub struct RegisterPreset {
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

    type Preset = RegisterPreset;

    /// 预置: 发验证码 + 从 MailHog 读回真实验证码(独立命名空间避免读旧邮件)
    async fn preset(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Preset, Self::Error> {
        let username = format!("e2e_reg_{}", task.index);
        let email = format!("e2e_reg_{}@test.com", task.index);

        ctx.client
            .post("/auth/verification-codes", json!({ "email": email }))
            .await?;
        let code = ctx.wait_mailhog_code(&email).await?;
        Ok(RegisterPreset {
            username,
            email,
            code,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        preset: &Self::Preset,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .post(
                "/auth/register",
                json!({
                    "username": preset.username,
                    "email": preset.email,
                    "password": "Test123456",
                    "confirmPassword": "Test123456",
                    "nickname": "E2E",
                    "inviterCode": "DRIFTC",
                    "emailVerifyCode": preset.code,
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
        preset: &Self::Preset,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 回查: 新用户确实落库
        let exists = auth::user::Entity::find()
            .filter(auth::user::Column::Username.eq(&preset.username))
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

    type Preset = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _preset: &Self::Preset,
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
        _preset: &Self::Preset,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.code == 400)
    }
}

register_scenario!(
    RegisterInvalidCodeScenario,
    mode = memseek_test::RunMode::Times(32)
);
