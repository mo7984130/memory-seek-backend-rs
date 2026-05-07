pub mod auth;
pub mod user;

use std::sync::Arc;

use async_trait::async_trait;

use crate::metrics::MetricsRecorder;
use ::auth::client::AuthClient;

#[async_trait]
pub trait Scenario: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, client: &AuthClient, recorder: &MetricsRecorder) -> anyhow::Result<()>;
}

pub struct WeightedScenario {
    pub scenario: Arc<dyn Scenario>,
    pub weight: u32,
}
