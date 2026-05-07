use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;

use ::auth::client::{AuthClient, TokenStore};
use crate::metrics::MetricsRecorder;
use crate::scenarios::Scenario;

pub struct TokenRefreshScenario {
    user_ids: Vec<i64>,
    token_store: Arc<TokenStore>,
}

impl TokenRefreshScenario {
    pub fn new(user_ids: Vec<i64>, token_store: Arc<TokenStore>) -> Self {
        Self {
            user_ids,
            token_store,
        }
    }
}

#[async_trait]
impl Scenario for TokenRefreshScenario {
    fn name(&self) -> &str {
        "auth/token_refresh"
    }

    async fn execute(&self, _client: &AuthClient, recorder: &MetricsRecorder) -> anyhow::Result<()> {
        let idx = rand::random_range(0..self.user_ids.len());
        let user_id = self.user_ids[idx];

        let start = Instant::now();
        let result = self.token_store.get_auth(user_id).await;
        recorder.record(start.elapsed(), result.is_ok());
        Ok(())
    }
}
