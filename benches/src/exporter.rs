use std::fs;
use std::path::Path;
use std::time::SystemTime;

use serde::Serialize;

use crate::metrics::MetricsSnapshot;

#[derive(Serialize)]
struct StageResult {
    concurrency: usize,
    total_requests: u64,
    success: u64,
    errors: u64,
    error_rate: f64,
    rps: f64,
    latency_p50_ms: f64,
    latency_p90_ms: f64,
    latency_p99_ms: f64,
    latency_max_ms: f64,
    latency_min_ms: f64,
    latency_mean_ms: f64,
}

#[derive(Serialize)]
struct BenchReport {
    timestamp: String,
    stages: Vec<StageResult>,
}

impl StageResult {
    fn from_snapshot(concurrency: usize, snapshot: &MetricsSnapshot, duration_secs: u64) -> Self {
        let rps = if duration_secs > 0 {
            snapshot.total as f64 / duration_secs as f64
        } else {
            0.0
        };
        let error_rate = if snapshot.total > 0 {
            snapshot.errors as f64 / snapshot.total as f64 * 100.0
        } else {
            0.0
        };

        Self {
            concurrency,
            total_requests: snapshot.total,
            success: snapshot.success,
            errors: snapshot.errors,
            error_rate,
            rps,
            latency_p50_ms: snapshot.p50.as_secs_f64() * 1000.0,
            latency_p90_ms: snapshot.p90.as_secs_f64() * 1000.0,
            latency_p99_ms: snapshot.p99.as_secs_f64() * 1000.0,
            latency_max_ms: snapshot.max.as_secs_f64() * 1000.0,
            latency_min_ms: snapshot.min.as_secs_f64() * 1000.0,
            latency_mean_ms: snapshot.mean.as_secs_f64() * 1000.0,
        }
    }
}

pub fn export_results(
    results: &[(usize, MetricsSnapshot)],
    stage_duration_secs: u64,
    output_dir: &str,
    formats: &[String],
) -> anyhow::Result<()> {
    fs::create_dir_all(output_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    let stage_results: Vec<StageResult> = results
        .iter()
        .map(|(c, s)| StageResult::from_snapshot(*c, s, stage_duration_secs))
        .collect();

    let report = BenchReport {
        timestamp: format!("{:?}", SystemTime::now()),
        stages: stage_results,
    };

    for format in formats {
        match format.as_str() {
            "json" => {
                let path = Path::new(output_dir).join(format!("{}.json", timestamp));
                let json = serde_json::to_string_pretty(&report)?;
                fs::write(&path, &json)?;
                println!("Exported JSON: {}", path.display());

                // Also write summary.json
                let summary = Path::new(output_dir).join("summary.json");
                fs::write(&summary, &json)?;
            }
            "csv" => {
                let path = Path::new(output_dir).join(format!("{}.csv", timestamp));
                let mut csv = String::from("concurrency,total_requests,success,errors,error_rate,rps,p50_ms,p90_ms,p99_ms,max_ms,min_ms,mean_ms\n");
                for s in &report.stages {
                    csv.push_str(&format!(
                        "{},{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
                        s.concurrency,
                        s.total_requests,
                        s.success,
                        s.errors,
                        s.error_rate,
                        s.rps,
                        s.latency_p50_ms,
                        s.latency_p90_ms,
                        s.latency_p99_ms,
                        s.latency_max_ms,
                        s.latency_min_ms,
                        s.latency_mean_ms,
                    ));
                }
                fs::write(&path, &csv)?;
                println!("Exported CSV: {}", path.display());
            }
            _ => {
                eprintln!("Unknown export format: {}", format);
            }
        }
    }

    Ok(())
}
