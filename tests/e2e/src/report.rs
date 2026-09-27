//! 运行结果汇总: 判定(退出码)与机器可读 JSON 报告。
//!
//! 文本可视化报告由 `memseek_test` 的 `Report` 渲染;本模块只负责:
//! 1. 汇总各场景的轮次/失败/超时, 按阈值给出通过与否(供 CI 退出码使用);
//! 2. 输出 JSON 报告, 供跨 run 的基线比对与压测报告生成。
//!
//! 注意: `setup` / `validate` / `teardown` 的耗时不计入延迟分位(`run` 之外),
//! 但会拖慢轮次节奏, 因此 RPS 反映的是"含准备与收尾"的整体吞吐。

use std::{
    fmt::Write as _,
    time::{SystemTime, UNIX_EPOCH},
};

use memseek_test::ScenarioReport;
use serde::Serialize;

use crate::config::RunConfig;

/// JSON 报告的结构版本, 便于将来做基线兼容判断。
const SCHEMA_VERSION: u32 = 1;

/// 汇总判定结果。
#[derive(Debug)]
pub struct Verdict {
    pub passed: bool,
    /// 未通过原因(按顺序: 中断 / 失败率 / 超时率 / 无有效轮次)
    pub reasons: Vec<String>,
    /// 失败率 = (run 失败 + setup 失败 + validate 未通过) / 轮次;
    /// setup 失败不计入轮次, 因此该值可能大于 1
    pub failure_rate: f64,
    /// 超时率 = timeouts / 轮次
    pub timeout_rate: f64,
    /// teardown 失败数: 只告警, 不参与判定
    pub teardown_failures: u64,
    pub times: u64,
}

impl Verdict {
    /// 汇总所有场景并按 [`RunConfig`] 的阈值判定。
    pub fn evaluate(reports: &[ScenarioReport], run: &RunConfig) -> Self {
        let times: u64 = reports.iter().map(|r| r.times).sum();
        let failures: u64 = reports.iter().map(|r| r.failures).sum();
        let validate_failures: u64 = reports.iter().map(|r| r.validate_failures).sum();
        let timeouts: u64 = reports.iter().map(|r| r.timeouts).sum();
        let teardown_failures: u64 = reports.iter().map(|r| r.teardown_failures).sum();

        let rate = |part: u64| part as f64 / times.max(1) as f64;
        let failure_rate = rate(failures + validate_failures);
        let timeout_rate = rate(timeouts);

        let mut reasons = Vec::new();
        if times == 0 {
            reasons.push("没有任何有效轮次(times = 0)".to_string());
        }
        let interrupted: Vec<&str> = reports
            .iter()
            .filter(|r| r.interrupted)
            .map(|r| r.name.as_ref())
            .collect();
        if !interrupted.is_empty() {
            reasons.push(format!(
                "场景被优雅关闭提前中断: {}",
                interrupted.join(", ")
            ));
        }
        if failure_rate > run.max_failure_rate {
            reasons.push(format!(
                "失败率 {:.4} 超过上限 {:.4}(失败 {failures} 含 setup 失败——不计入轮次, 校验未通过 {validate_failures} / 轮次 {times})",
                failure_rate, run.max_failure_rate
            ));
        }
        if timeout_rate > run.max_timeout_rate {
            reasons.push(format!(
                "超时率 {:.4} 超过上限 {:.4} (超时 {timeouts} / 轮次 {times})",
                timeout_rate, run.max_timeout_rate
            ));
        }

        Self {
            passed: reasons.is_empty(),
            reasons,
            failure_rate,
            timeout_rate,
            teardown_failures,
            times,
        }
    }

    /// 汇总行 + 未通过原因(供 CI 日志直接阅读)。
    pub fn summary(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "汇总: 轮次 {} | 失败率 {:.4} | 超时率 {:.4} | 收尾失败 {}",
            self.times, self.failure_rate, self.timeout_rate, self.teardown_failures
        );
        if self.teardown_failures > 0 && self.passed {
            let _ = writeln!(
                out,
                "注意: 收尾失败 {} 次(不影响判定, 但可能有资源泄漏; 详见错误明细 teardown:*)",
                self.teardown_failures
            );
        }
        if self.passed {
            let _ = writeln!(out, "判定: PASS");
        } else {
            let _ = writeln!(out, "判定: FAIL");
            for reason in &self.reasons {
                let _ = writeln!(out, "  - {reason}");
            }
        }
        out
    }
}

/// 完整 JSON 报告(写入文件供 CI 归档与基线比对)。
#[derive(Debug, Serialize)]
pub struct JsonReport {
    pub schema_version: u32,
    pub generated_at_ms: u128,
    pub environment: JsonEnvironment,
    pub overall: JsonOverall,
    pub scenarios: Vec<JsonScenario>,
}

/// 本次运行的环境与参数(基线比对要求两边口径一致)。
#[derive(Debug, Serialize)]
pub struct JsonEnvironment {
    pub server_url: String,
    /// `times` 或 `duration`
    pub mode: String,
    pub concurrency: u64,
    /// `mode=times` 的计划轮次(其它模式下为 0)
    pub planned_times: u64,
    /// `mode=duration` 的计划时长(秒, 其它模式下为 0)
    pub planned_duration_secs: u64,
    /// 是否执行了前置准备(preprea)
    pub prepared: bool,
    /// 场景白名单(空字符串 = 全部场景)
    pub scenario_filter: String,
    pub max_failure_rate: f64,
    pub max_timeout_rate: f64,
}

/// 全场景汇总。
#[derive(Debug, Serialize)]
pub struct JsonOverall {
    pub times: u64,
    pub success: u64,
    pub failures: u64,
    pub validate_failures: u64,
    pub teardown_failures: u64,
    pub timeouts: u64,
    pub interrupted: bool,
    /// 各场景墙钟耗时之和(毫秒)
    pub elapsed_ms: f64,
    pub rps: f64,
    /// 失败率(setup 失败计入分子但不计入轮次, 因此可能 > 1)
    pub failure_rate: f64,
    pub timeout_rate: f64,
    pub passed: bool,
}

/// 单场景指标(耗时统一毫秒, 便于跨语言脚本处理)。
#[derive(Debug, Serialize)]
pub struct JsonScenario {
    pub name: String,
    pub times: u64,
    pub elapsed_ms: f64,
    pub rps: f64,
    pub min_ms: f64,
    pub avg_ms: f64,
    pub max_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub success: u64,
    pub failures: u64,
    pub validate_success: u64,
    pub validate_failures: u64,
    /// 收尾尝试次数与失败数(失败不计入 `failures`)
    pub teardown_count: u64,
    pub teardown_failures: u64,
    pub timeouts: u64,
    pub interrupted: bool,
    /// 错误分类计数(`teardown:*` 为收尾失败)
    pub errors: std::collections::BTreeMap<String, u64>,
}

impl JsonReport {
    /// 由场景报告与本次运行参数构造 JSON 报告。
    pub fn build(
        reports: &[ScenarioReport],
        run: &RunConfig,
        server_url: String,
        prepared: bool,
        verdict: &Verdict,
    ) -> Self {
        let scenarios: Vec<JsonScenario> = reports.iter().map(JsonScenario::from).collect();

        let times: u64 = reports.iter().map(|r| r.times).sum();
        let elapsed_ms: f64 = reports.iter().map(|r| ms(r.elapsed)).sum();
        let overall = JsonOverall {
            times,
            success: reports.iter().map(|r| r.success).sum(),
            failures: reports.iter().map(|r| r.failures).sum(),
            validate_failures: reports.iter().map(|r| r.validate_failures).sum(),
            teardown_failures: verdict.teardown_failures,
            timeouts: reports.iter().map(|r| r.timeouts).sum(),
            interrupted: reports.iter().any(|r| r.interrupted),
            elapsed_ms,
            rps: rps(times, elapsed_ms),
            failure_rate: verdict.failure_rate,
            timeout_rate: verdict.timeout_rate,
            passed: verdict.passed,
        };

        Self {
            schema_version: SCHEMA_VERSION,
            generated_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default(),
            environment: JsonEnvironment {
                server_url,
                mode: match run.mode {
                    crate::config::RunModeKind::Times => "times".to_string(),
                    crate::config::RunModeKind::Duration => "duration".to_string(),
                },
                concurrency: run.concurrency,
                planned_times: match run.mode {
                    crate::config::RunModeKind::Times => run.times,
                    crate::config::RunModeKind::Duration => 0,
                },
                planned_duration_secs: match run.mode {
                    crate::config::RunModeKind::Duration => run.duration_secs,
                    crate::config::RunModeKind::Times => 0,
                },
                prepared,
                scenario_filter: run.scenarios.clone(),
                max_failure_rate: run.max_failure_rate,
                max_timeout_rate: run.max_timeout_rate,
            },
            overall,
            scenarios,
        }
    }

    /// 序列化为带缩进的 JSON 文本。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl From<&ScenarioReport> for JsonScenario {
    fn from(r: &ScenarioReport) -> Self {
        Self {
            name: r.name.to_string(),
            times: r.times,
            elapsed_ms: ms(r.elapsed),
            rps: rps(r.times, ms(r.elapsed)),
            min_ms: ms(r.min),
            avg_ms: ms(r.avg),
            max_ms: ms(r.max),
            p50_ms: ms(r.p50),
            p95_ms: ms(r.p95),
            p99_ms: ms(r.p99),
            success: r.success,
            failures: r.failures,
            validate_success: r.validate_success,
            validate_failures: r.validate_failures,
            teardown_count: r.teardown_success + r.teardown_failures,
            teardown_failures: r.teardown_failures,
            timeouts: r.timeouts,
            interrupted: r.interrupted,
            errors: r
                .error_map
                .iter()
                .map(|(kind, count)| (kind.to_string(), *count))
                .collect(),
        }
    }
}

fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn rps(times: u64, elapsed_ms: f64) -> f64 {
    if elapsed_ms <= 0.0 {
        return 0.0;
    }
    times as f64 / (elapsed_ms / 1000.0)
}
