use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, SelectExt};
use serde_json::json;
use types::auth::{self, LoginResponse, RefreshAccessTokenResponse, user::UserId};

use crate::context::Context;

/// 刷新 access_token: 登录拿 refresh_token 后调用刷新, 校验确实签发了新 token。
pub struct RefreshOutput {
    pub user_id: UserId,
    pub refresh_token: String,
    pub old_access_token: String,
    pub new_access_token: String,
}

#[derive(Default)]
pub struct RefreshScenario;
impl Scenario for RefreshScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = RefreshOutput;

    async fn run(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Output, Self::Error> {
        // 前置: 登录获取凭据
        let login: SucR<LoginResponse> = ctx
            .client
            .post(
                "/auth/login",
                json!({ "account": format!("loadtest_{}", task.index + 1), "password": "Test123456" }),
            )
            .await?
            .json()
            .await?;

        // 刷新: 凭据从请求头读取
        let resp: SucR<RefreshAccessTokenResponse> = ctx
            .client
            .request(reqwest::Method::POST, "/auth/token")
            .header("x-user-id", &login.data.user.id.to_string())
            .header("x-refresh-token", &login.data.refresh_token)
            .json_unwrap(&json!({}))
            .send()
            .await?
            .json()
            .await?;

        Ok(RefreshOutput {
            user_id: login.data.user.id,
            refresh_token: login.data.refresh_token,
            old_access_token: login.data.access_token,
            new_access_token: resp.data.access_token,
        })
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 确实签发了新 access_token
        let issued_new = output.new_access_token != output.old_access_token;
        // refresh_token 未被破坏: 库中仍与登录时一致
        let token_intact = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(output.user_id))
            .filter(auth::user::Column::RefreshToken.eq(&output.refresh_token))
            .exists(&ctx.db)
            .await
            .unwrap();
        Ok(issued_new && token_intact)
    }
}

register_scenario!(RefreshScenario, mode = memseek_test::RunMode::Times(32));

/// 刷新失败 - 伪造 refresh_token: 期望 401。
#[derive(Default)]
pub struct RefreshInvalidTokenScenario;
impl Scenario for RefreshInvalidTokenScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    async fn run(ctx: &Self::Ctx, _task: &TaskIndex) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/auth/token")
            // loadtest_1 的 id = 1 + 1 = 2
            .header("x-user-id", "2")
            .header("x-refresh-token", "forged_refresh_token")
            .json_unwrap(&json!({}))
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.code == 401)
    }
}

register_scenario!(
    RefreshInvalidTokenScenario,
    mode = memseek_test::RunMode::Times(32)
);
