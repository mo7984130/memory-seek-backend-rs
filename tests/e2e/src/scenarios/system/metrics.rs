//! Prometheus 指标暴露:`GET /metrics`(公开路由)。
//!
//! 目的:防埋点回归 —— 登录会记录 `auth:login:attempts`, 断言指标文本里存在
//! 服务级与领域级关键前缀。断言用**前缀子串**而非完整名:
//! exporter 只把非法字符(如 `.`)换成 `_`, 而 `:` 在 Prometheus 中合法会保留
//! (如 `server_build_info` vs `auth:login:attempts`), counter 另带 `_total` 后缀。

use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};

use crate::context::Context;

use super::super::user::session::{Session, login, user_account};

/// 指标访问前置:登录一次, 保证 `auth:login:*` 指标已产生。
#[derive(Default)]
pub struct MetricsScenario;

impl Scenario for MetricsScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = String;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &user_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(ctx.client.get("/metrics").await?.text().await?)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 服务级:构建信息(启动时上报)与 HTTP 请求计数(中间件记录, 登录请求已触发)
        let server_ok =
            output.contains("server_build_info") && output.contains("server_http_requests");
        // 领域级:登录成功会记录 `auth:login:attempts` / `auth:login:success`(保留冒号)
        let domain_ok = output.contains("auth:login");
        Ok(server_ok && domain_ok)
    }
}

register_scenario!(MetricsScenario, mode = memseek_test::RunMode::Times(32));
