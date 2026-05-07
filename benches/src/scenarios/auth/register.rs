use std::time::Instant;

use async_trait::async_trait;
use auth::models::RegisterRequest;

use ::auth::client::AuthClient;
use crate::metrics::MetricsRecorder;
use crate::scenarios::Scenario;

pub struct RegisterScenario {
    base_username: String,
    counter: std::sync::atomic::AtomicU64,
}

impl RegisterScenario {
    pub fn new(base_username: &str) -> Self {
        Self {
            base_username: base_username.to_string(),
            counter: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl Scenario for RegisterScenario {
    fn name(&self) -> &str {
        "auth/register"
    }

    async fn execute(&self, client: &AuthClient, recorder: &MetricsRecorder) -> anyhow::Result<()> {
        let n = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let username = format!("{}_{:08x}", self.base_username, n);
        let email = format!("{}@bench.test", username);

        let start = Instant::now();
        let resp = client
            .raw()
            .post(format!("{}/auth/register", client.base_url()))
            .json(&RegisterRequest {
                username: username.clone(),
                email,
                password: "Bench@12345".to_string(),
                nickname: username,
                inviter_code: "000000".to_string(),
                email_verify_code: "000000".to_string(),
            })
            .send()
            .await?;

        let success = resp.status().is_success();
        let _body: serde_json::Value = resp.json().await?;
        recorder.record(start.elapsed(), success);
        Ok(())
    }
}
