use memory_seek_api::api;
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode, TeardownMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, SelectExt};
use types_identity::auth::{self, RegisterRequest, SendEmailCodeRequest};
use types_identity::user::UserInfo;

use crate::context::Context;
use crate::error::{E2eError, status};

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

    type Error = E2eError;

    type Output = UserInfo;

    type Setup = RegisterSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    /// 预置: 发验证码 + 从 MailHog 读回真实验证码(独立命名空间避免读旧邮件)
    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let username = format!("e2e_reg_{}_{}", task.index, task.round);
        let email = format!("e2e_reg_{}_{}@test.com", task.index, task.round);

        api::auth::send_email_code(
            &ctx.api_client(),
            &SendEmailCodeRequest {
                email: email.clone(),
            },
        )
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
        let request = RegisterRequest {
            username: setup.username.clone(),
            email: setup.email.clone(),
            password: "Test123456".to_owned(),
            confirm_password: "Test123456".to_owned(),
            nickname: "E2E".to_owned(),
            inviter_code: "DRIFTC".to_owned(),
            email_verify_code: setup.code.clone(),
        };
        Ok(api::auth::register(&ctx.api_client(), &request).await?)
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

    /// 收尾: 删除本轮注册的账号与验证码邮件。
    /// 长跑下每轮新增一个账号(无"注销账号"接口, 直连数据库清理),
    /// 邮件每轮一封(按地址清, 不影响并发跑着的其它场景)
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn teardown(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        // 邮件无论成败都清: run 失败也可能已经投递过
        if let Err(error) = ctx.mailhog_purge(&setup.email).await {
            tracing::warn!(email = %setup.email, %error, "清理验证码邮件失败");
        }
        if !matches!(result, Some(Ok(_))) {
            // 未注册成功 = 没有账号可删
            return Ok(());
        }
        if let Err(error) = auth::user::Entity::delete_many()
            .filter(auth::user::Column::Username.eq(&setup.username))
            .exec(&ctx.db)
            .await
        {
            tracing::warn!(username = %setup.username, %error, "清理注册账号失败");
            return Err(E2eError::internal(format!("删除注册账号失败: {error}")));
        }
        Ok(())
    }
}

register_scenario!(RegisterScenario);

/// 注册失败 - 邮箱验证码错误: 期望 400。
#[derive(Default)]
pub struct RegisterInvalidCodeScenario;
impl Scenario for RegisterInvalidCodeScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let request = RegisterRequest {
            username: format!("e2e_inv_{}", task.index),
            email: format!("e2e_inv_{}@test.com", task.index),
            password: "Test123456".to_owned(),
            confirm_password: "Test123456".to_owned(),
            nickname: "E2E".to_owned(),
            inviter_code: "DRIFTC".to_owned(),
            email_verify_code: "000000".to_owned(),
        };
        status(api::auth::register(&ctx.api_client(), &request).await)
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

register_scenario!(RegisterInvalidCodeScenario);
