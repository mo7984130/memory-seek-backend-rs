use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, SelectExt};
use serde_json::json;
use types::auth::{self, LoginResponse, RefreshAccessTokenResponse, user::UserId};

use crate::context::Context;

/// 刷新 access_token: setup 每任务登录一次拿凭据, 每轮 run 只做刷新,
/// 校验确实签发了新 token 且 refresh_token 未被破坏。
pub struct RefreshSetup {
    pub user_id: UserId,
    pub refresh_token: String,
    pub old_access_token: String,
}

impl Default for RefreshSetup {
    fn default() -> Self {
        Self {
            user_id: UserId(0),
            refresh_token: String::new(),
            old_access_token: String::new(),
        }
    }
}

#[derive(Default)]
pub struct RefreshScenario;
impl Scenario for RefreshScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<RefreshAccessTokenResponse>;

    type Setup = RefreshSetup;

    /// 预置: 每任务登录一次, 产出凭据供本轮刷新使用
    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let login: SucR<LoginResponse> = ctx
            .client
            .request(reqwest::Method::POST, "/auth/login")
            .json_unwrap(&json!({
                "account": format!("loadtest_{}", task.index + 1),
                "password": "Test123456"
            }))
            .send_checked()
            .await?
            .json()
            .await?;
        Ok(RefreshSetup {
            user_id: login.data.user.id,
            refresh_token: login.data.refresh_token,
            old_access_token: login.data.access_token,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 刷新: 凭据从请求头读取
        ctx.client
            .request(reqwest::Method::POST, "/auth/token")
            .header("x-user-id", &setup.user_id.to_string())
            .header("x-refresh-token", &setup.refresh_token)
            .json_unwrap(&json!({}))
            .send()
            .await?
            .json()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 确实签发了新 access_token
        let issued_new = output.data.access_token != setup.old_access_token;
        // refresh_token 未被破坏: 库中仍与登录时一致
        let token_intact = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.user_id))
            .filter(auth::user::Column::RefreshToken.eq(&setup.refresh_token))
            .exists(&ctx.db)
            .await
            .unwrap();
        Ok(issued_new && token_intact)
    }
}

register_scenario!(RefreshScenario);

/// 刷新失败 - 伪造 refresh_token: 期望 401。
#[derive(Default)]
pub struct RefreshInvalidTokenScenario;
impl Scenario for RefreshInvalidTokenScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // loadtest_{index+1} 的 id = index + 2(种子 id 从 2 开始, admin 占 id=1)
        let user_id = task.index + 2;
        ctx.client
            .request(reqwest::Method::POST, "/auth/token")
            .header("x-user-id", &user_id.to_string())
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
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.code == 401)
    }
}

register_scenario!(RefreshInvalidTokenScenario);
