use std::time::Instant;

use async_trait::async_trait;
use auth::models::LoginRequest;
use entities::user::UserDTO;

use ::auth::client::AuthClient;
use crate::config::UserCredential;
use crate::metrics::MetricsRecorder;
use crate::scenarios::Scenario;

/// Wrapper matching the API response: { code, msg, data }
#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct ApiResponse<T> {
    code: u16,
    data: Option<T>,
}

pub struct LoginScenario {
    users: Vec<UserCredential>,
}

impl LoginScenario {
    pub fn new(users: Vec<UserCredential>) -> Self {
        Self { users }
    }
}

#[async_trait]
impl Scenario for LoginScenario {
    fn name(&self) -> &str {
        "auth/login"
    }

    async fn execute(&self, client: &AuthClient, recorder: &MetricsRecorder) -> anyhow::Result<()> {
        let idx = rand::random_range(0..self.users.len());
        let cred = &self.users[idx];

        let start = Instant::now();
        let resp = client
            .raw()
            .post(format!("{}/auth/login", client.base_url()))
            .json(&LoginRequest {
                account: cred.account.clone(),
                password: cred.password.clone(),
            })
            .send()
            .await?;

        let success = resp.status().is_success();
        let _body: ApiResponse<UserDTO> = resp.json().await?;
        recorder.record(start.elapsed(), success);
        Ok(())
    }
}
