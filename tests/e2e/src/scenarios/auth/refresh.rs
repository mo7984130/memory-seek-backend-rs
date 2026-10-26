use memory_seek_api::api;
use memseek_test::{TaskIndex, register_scenario, scenario::Scenario};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, SelectExt};
use types_identity::auth::user::UserId;
use types_identity::auth::{self, LoginRequest, RefreshAccessTokenResponse};

use crate::context::Context;
use crate::error::{E2eError, status};

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

    type Error = E2eError;

    type Output = RefreshAccessTokenResponse;

    type Setup = RefreshSetup;

    /// 预置: 每任务登录一次, 产出凭据供本轮刷新使用
    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let request = LoginRequest {
            account: format!("loadtest_{}", task.index + 1),
            password: "Test123456".to_owned(),
        };
        let login = api::auth::login(&ctx.api_client(), &request).await?;
        Ok(RefreshSetup {
            user_id: login.user.id,
            refresh_token: login.refresh_token,
            old_access_token: login.access_token,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 刷新: 凭据作为参数传入(api-rs 内部经请求头发送, 不写回存储)
        Ok(
            api::auth::refresh_token(&ctx.api_client(), setup.user_id, &setup.refresh_token)
                .await?,
        )
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 确实签发了新 access_token
        let issued_new = output.access_token != setup.old_access_token;
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

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // loadtest_{index+1} 的 id = index + 2(种子 id 从 2 开始, admin 占 id=1)
        let user_id = UserId((task.index + 2) as i64);
        status(api::auth::refresh_token(&ctx.api_client(), user_id, "forged_refresh_token").await)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 401)
    }
}

register_scenario!(RefreshInvalidTokenScenario);
