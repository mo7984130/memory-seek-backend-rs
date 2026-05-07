use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

use ::auth::client::{AuthClient, TokenStore};
use crate::config::{BenchConfig, StaircaseConfig};
use crate::metrics::MetricsRecorder;
use crate::reporter;
use crate::scenarios::{Scenario, WeightedScenario};

pub struct Runner {
    config: BenchConfig,
    client: Arc<AuthClient>,
    token_store: Arc<TokenStore>,
}

impl Runner {
    pub fn new(config: BenchConfig, client: Arc<AuthClient>, token_store: Arc<TokenStore>) -> Self {
        Self {
            config,
            client,
            token_store,
        }
    }

    pub async fn run(&self, scenarios: Vec<WeightedScenario>) -> anyhow::Result<()> {
        // Warmup: login all users
        println!("Warming up: logging in {} users...", self.config.users.len());
        let credentials: Vec<(&str, &str)> = self.config.users.iter().map(|u| (u.account.as_str(), u.password.as_str())).collect();
        let _user_ids = self.token_store.warmup(&credentials).await?;
        println!("Warmup complete.");

        // Build weighted scenario list (cloned Arcs)
        let weighted = build_weighted_list(&scenarios);

        if self.config.bench.warmup_secs > 0 {
            println!(
                "Running warmup phase for {}s...",
                self.config.bench.warmup_secs
            );
            self.run_loop(&weighted, Duration::from_secs(self.config.bench.warmup_secs))
                .await;
            println!("Warmup phase complete.");
        }

        println!(
            "Running benchmark for {}s with concurrency {}...",
            self.config.bench.duration_secs, self.config.bench.concurrency
        );

        let recorder = Arc::new(MetricsRecorder::new());
        let semaphore = Arc::new(Semaphore::new(self.config.bench.concurrency));
        let deadline = Instant::now() + Duration::from_secs(self.config.bench.duration_secs);
        let mut handles = Vec::new();

        loop {
            if Instant::now() >= deadline {
                break;
            }

            let permit = semaphore.clone().acquire_owned().await?;
            let scenario_idx = rand::random_range(0..weighted.len());
            let scenario = weighted[scenario_idx].clone();
            let client = self.client.clone();
            let recorder = recorder.clone();

            let handle = tokio::spawn(async move {
                let _ = scenario.execute(&client, &recorder).await;
                drop(permit);
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        let snapshot = recorder.snapshot();
        reporter::print_report("aggregate", &snapshot);

        Ok(())
    }

    pub async fn run_staircase(
        &self,
        scenarios: Vec<WeightedScenario>,
        staircase: StaircaseConfig,
    ) -> anyhow::Result<Vec<(usize, crate::metrics::MetricsSnapshot)>> {
        // Warmup: login all users
        println!("Warming up: logging in {} users...", self.config.users.len());
        let credentials: Vec<(&str, &str)> = self.config.users.iter().map(|u| (u.account.as_str(), u.password.as_str())).collect();
        let _user_ids = self.token_store.warmup(&credentials).await?;
        println!("Warmup complete.");

        let weighted = build_weighted_list(&scenarios);

        if self.config.bench.warmup_secs > 0 {
            println!("Running warmup phase for {}s...", self.config.bench.warmup_secs);
            self.run_loop(&weighted, Duration::from_secs(self.config.bench.warmup_secs)).await;
            println!("Warmup phase complete.");
        }

        let scheduler = StaircaseScheduler::new(staircase, self.config.bench.clone());
        scheduler.run(&weighted, &self.client).await
    }

    async fn run_loop(&self, weighted: &[Arc<dyn Scenario>], duration: Duration) {
        let recorder = Arc::new(MetricsRecorder::new());
        let semaphore = Arc::new(Semaphore::new(self.config.bench.concurrency));
        let deadline = Instant::now() + duration;
        let mut handles = Vec::new();

        loop {
            if Instant::now() >= deadline {
                break;
            }

            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let scenario_idx = rand::random_range(0..weighted.len());
            let scenario = weighted[scenario_idx].clone();
            let client = self.client.clone();
            let recorder = recorder.clone();

            let handle = tokio::spawn(async move {
                let _ = scenario.execute(&client, &recorder).await;
                drop(permit);
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }
    }
}

/// Build a flat list of Arc-cloned scenarios based on weights.
fn build_weighted_list(scenarios: &[WeightedScenario]) -> Vec<Arc<dyn Scenario>> {
    let mut list = Vec::new();
    for ws in scenarios {
        for _ in 0..ws.weight {
            list.push(ws.scenario.clone());
        }
    }
    list
}

pub struct StaircaseScheduler {
    config: StaircaseConfig,
    bench_config: crate::config::RunConfig,
}

impl StaircaseScheduler {
    pub fn new(staircase: StaircaseConfig, bench_config: crate::config::RunConfig) -> Self {
        Self {
            config: staircase,
            bench_config,
        }
    }

    pub async fn run(
        &self,
        scenarios: &[Arc<dyn Scenario>],
        client: &Arc<AuthClient>,
    ) -> anyhow::Result<Vec<(usize, crate::metrics::MetricsSnapshot)>> {
        let mut results = Vec::new();

        for (i, stage) in self.config.stages.iter().enumerate() {
            println!(
                "\n=== Stage {}/{}: concurrency={} duration={}s ===",
                i + 1,
                self.config.stages.len(),
                stage.concurrency,
                self.config.stage_duration_secs
            );

            let recorder = Arc::new(crate::metrics::MetricsRecorder::new());
            let semaphore = Arc::new(Semaphore::new(stage.concurrency));
            let deadline = Instant::now() + Duration::from_secs(self.config.stage_duration_secs);
            let mut handles = Vec::new();

            loop {
                if Instant::now() >= deadline {
                    break;
                }

                let permit = semaphore.clone().acquire_owned().await?;
                let scenario_idx = rand::random_range(0..scenarios.len());
                let scenario = scenarios[scenario_idx].clone();
                let client = client.clone();
                let recorder = recorder.clone();

                let handle = tokio::spawn(async move {
                    let _ = scenario.execute(&client, &recorder).await;
                    drop(permit);
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }

            let snapshot = recorder.snapshot();
            reporter::print_report(&format!("stage_{}", stage.concurrency), &snapshot);
            results.push((stage.concurrency, snapshot));

            // Cooldown between stages
            if i < self.config.stages.len() - 1 && self.config.cooldown_secs > 0 {
                println!("Cooling down for {}s...", self.config.cooldown_secs);
                tokio::time::sleep(Duration::from_secs(self.config.cooldown_secs)).await;
            }
        }

        Ok(results)
    }
}
