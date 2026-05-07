use std::time::Instant;

use async_trait::async_trait;

use crate::config::UserCredential;
use crate::metrics::MetricsRecorder;
use crate::scenarios::Scenario;
use ::auth::client::AuthClient;

pub struct GetProfileScenario {
    users: Vec<UserCredential>,
}

impl GetProfileScenario {
    pub fn new(users: Vec<UserCredential>) -> Self {
        Self { users }
    }
}

#[async_trait]
impl Scenario for GetProfileScenario {
    fn name(&self) -> &str {
        "user/get_profile"
    }

    async fn execute(&self, client: &AuthClient, recorder: &MetricsRecorder) -> anyhow::Result<()> {
        let idx = rand::random_range(0..self.users.len());
        let user_id = self.get_user_id(client, &self.users[idx].account).await?;

        let start = Instant::now();
        let resp = client.get("/user/info", user_id).await?.send().await?;

        let success = resp.status().is_success();
        let _body: serde_json::Value = resp.json().await?;
        recorder.record(start.elapsed(), success);
        Ok(())
    }
}

impl GetProfileScenario {
    async fn get_user_id(&self, client: &AuthClient, account: &str) -> anyhow::Result<i64> {
        let resp = client
            .raw()
            .post(format!("{}/auth/login", client.base_url()))
            .json(&serde_json::json!({
                "account": account,
                "password": "123456abc"
            }))
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;
        let user_id = body["data"]["id"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        Ok(user_id)
    }
}
