use memory_seek_api::api;
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode, TeardownMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_identity::auth::{RegisterRequest, SendEmailCodeRequest, user};

use crate::context::Context;
use crate::error::E2eError;

/// 发送邮箱验证码: 校验 MailHog 确实收到邮件, 且邮件 code 与 server
/// 实际存储(Redis)的一致(真实链路投递)。
#[derive(Default)]
pub struct SendCodeScenario;
impl Scenario for SendCodeScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = String;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let email = format!("e2e_{}@test.com", task.index);
        let request = SendEmailCodeRequest {
            email: email.clone(),
        };
        api::auth::send_email_code(&ctx.api_client(), &request).await?;
        Ok(email)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 轮询等待邮件到达, 并校验邮件正文 code 与 server 实际存储(Redis)的一致
        for _ in 0..10 {
            let mail_code = match ctx.mailhog_latest_code(output).await {
                Ok(code) => code,
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
            };
            if let Some(stored) = ctx.redis_email_code(output).await {
                return Ok(mail_code == stored);
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        Ok(false)
    }

    /// 收尾: 删除本轮发送的验证码邮件。
    /// 邮箱按 task 复用且每轮发一封, 不回收会让 MailHog 无限膨胀(拖慢全部邮件校验)
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn teardown(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let email = format!("e2e_{}@test.com", task.index);
        let deleted = ctx.mailhog_purge(&email).await?;
        tracing::debug!(%email, deleted, "清理验证码邮件");
        Ok(())
    }
}

register_scenario!(SendCodeScenario);

/// 已注册邮箱不能再次请求注册验证码。
#[derive(Default)]
pub struct SendCodeRegisteredEmailScenario;

#[derive(Default)]
pub struct SendCodeRegisteredEmailSetup {
    username: String,
    email: String,
}

impl Scenario for SendCodeRegisteredEmailScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = SendCodeRegisteredEmailSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let username = format!("e2e_code_existing_{}_{}", task.index, task.round);
        let email = format!("{username}@test.com");

        api::auth::send_email_code(
            &ctx.api_client(),
            &SendEmailCodeRequest {
                email: email.clone(),
            },
        )
        .await?;
        let code = ctx.wait_mailhog_code(&email).await?;

        api::auth::register(
            &ctx.api_client(),
            &RegisterRequest {
                username: username.clone(),
                email: email.clone(),
                password: "Test123456".to_owned(),
                confirm_password: "Test123456".to_owned(),
                nickname: "E2E".to_owned(),
                inviter_code: "DRIFTC".to_owned(),
                email_verify_code: code,
            },
        )
        .await?;

        // 清掉注册时的邮件,以便验证重复请求没有再次投递。
        ctx.mailhog_purge(&email).await?;

        Ok(SendCodeRegisteredEmailSetup { username, email })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(crate::error::status(
            api::auth::send_email_code(
                &ctx.api_client(),
                &SendEmailCodeRequest {
                    email: setup.email.clone(),
                },
            )
            .await,
        )?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let mail_not_sent = ctx.mailhog_latest_code(&setup.email).await.is_err();
        let code_not_stored = ctx.redis_email_code(&setup.email).await.is_none();
        Ok(*output == 400 && mail_not_sent && code_not_stored)
    }

    async fn teardown(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        ctx.mailhog_purge(&setup.email).await?;
        user::Entity::delete_many()
            .filter(user::Column::Username.eq(&setup.username))
            .exec(&ctx.db)
            .await
            .map_err(|error| E2eError::internal(format!("清理测试用户失败: {error}")))?;
        Ok(())
    }
}

register_scenario!(SendCodeRegisteredEmailScenario);
