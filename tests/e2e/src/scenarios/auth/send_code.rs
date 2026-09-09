use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use serde_json::json;

use crate::context::Context;

/// 发送邮箱验证码: 校验 MailHog 确实收到邮件(真实链路投递)。
#[derive(Default)]
pub struct SendCodeScenario;
impl Scenario for SendCodeScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = String;

    async fn run(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Output, Self::Error> {
        let email = format!("e2e_{}@test.com", task.index);
        ctx.client
            .post("/auth/verification-codes", json!({ "email": email }))
            .await?;
        Ok(email)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 轮询等待邮件到达(异步投递)
        for _ in 0..10 {
            if let Ok(code) = ctx.mailhog_latest_code(output).await {
                return Ok(!code.is_empty());
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        Ok(false)
    }
}

register_scenario!(SendCodeScenario, mode = memseek_test::RunMode::Times(32));
