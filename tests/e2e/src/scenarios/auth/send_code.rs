use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::{Scenario, TeardownMode},
};
use serde_json::json;

use crate::context::Context;

/// 发送邮箱验证码: 校验 MailHog 确实收到邮件, 且邮件 code 与 server
/// 实际存储(Redis)的一致(真实链路投递)。
#[derive(Default)]
pub struct SendCodeScenario;
impl Scenario for SendCodeScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = String;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let email = format!("e2e_{}@test.com", task.index);
        ctx.client
            .request(reqwest::Method::POST, "/auth/verification-codes")
            .json_unwrap(&json!({ "email": email }))
            .send_checked()
            .await?;
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
