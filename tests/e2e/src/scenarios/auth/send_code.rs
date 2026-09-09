use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
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

    type Preset = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _preset: &Self::Preset,
    ) -> Result<Self::Output, Self::Error> {
        let email = format!("e2e_{}@test.com", task.index);
        ctx.client
            .post("/auth/verification-codes", json!({ "email": email }))
            .await?;
        Ok(email)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _preset: &Self::Preset,
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
}

register_scenario!(SendCodeScenario, mode = memseek_test::RunMode::Times(32));
