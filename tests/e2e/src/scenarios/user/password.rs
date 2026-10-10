use memory_seek_api::{Method, api};
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_identity::auth;
use types_identity::user::ChangePasswordParam;

use crate::context::Context;
use crate::error::{E2eError, status};

use super::session::{
    NEW_PASSWORD, PASSWORD, Session, login, login_with, pwd_account, user_account,
};

/// 修改密码: 改密后服务端会登出 —— 旧 token 失效、refresh_token 清空、新密码可登录.
#[derive(Default)]
pub struct ChangePasswordScenario;

impl Scenario for ChangePasswordScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = Session;

    // 改密会改变账号密码状态, Round 模式下每轮 setup 重新登录,
    // validate 中将密码还原回 PASSWORD 以便下一轮复用.
    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &pwd_account(task.index)).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let param = ChangePasswordParam {
            old_password: PASSWORD.to_owned(),
            new_password: NEW_PASSWORD.to_owned(),
            confirm_password: NEW_PASSWORD.to_owned(),
        };
        api::user::change_password(setup.client(), &param).await?;
        Ok(())
    }

    async fn validate(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        // 0. 先用新密码重新登录, 再把密码还原回 PASSWORD,
        //    保证 Round 模式下下一轮 setup 仍可用旧密码登录.
        let restored = match login_with(ctx, &pwd_account(task.index), NEW_PASSWORD).await {
            Ok(session) => {
                let param = ChangePasswordParam {
                    old_password: NEW_PASSWORD.to_owned(),
                    new_password: PASSWORD.to_owned(),
                    confirm_password: PASSWORD.to_owned(),
                };
                api::user::change_password(session.client(), &param)
                    .await
                    .is_ok()
            }
            Err(_) => false,
        };

        // 1. 旧 access_token 已失效(改密成功后服务端调用了 logout)
        let after = status(api::user::get_me(setup.client()).await)?;
        let revoked = after == 401;

        // 2. refresh_token 已清空
        let refresh_cleared = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.user_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some_and(|u| u.refresh_token.is_none());

        Ok(revoked && refresh_cleared && restored)
    }
}

register_scenario!(ChangePasswordScenario);

/// 旧密码错误: 期望 400.
#[derive(Default)]
pub struct ChangePasswordWrongOldScenario;

impl Scenario for ChangePasswordWrongOldScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &user_account(task.index)).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let param = ChangePasswordParam {
            old_password: "WrongPass1".to_owned(),
            new_password: NEW_PASSWORD.to_owned(),
            confirm_password: NEW_PASSWORD.to_owned(),
        };
        status(api::user::change_password(setup.client(), &param).await)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 400)
    }
}

register_scenario!(ChangePasswordWrongOldScenario);

/// 两次输入不一致: 期望 400(请求体校验阶段拦截).
#[derive(Default)]
pub struct ChangePasswordMismatchScenario;

impl Scenario for ChangePasswordMismatchScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &user_account(task.index)).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let param = ChangePasswordParam {
            old_password: PASSWORD.to_owned(),
            new_password: NEW_PASSWORD.to_owned(),
            confirm_password: "Different123".to_owned(),
        };
        status(api::user::change_password(setup.client(), &param).await)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 400)
    }
}

register_scenario!(ChangePasswordMismatchScenario);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct ChangePasswordUnauthorizedScenario;

impl Scenario for ChangePasswordUnauthorizedScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let client = ctx.api_client();
        let response = client
            .raw(Method::PATCH, "/user/password")
            .no_auth()
            .json(&serde_json::json!({
                "oldPassword": PASSWORD,
                "newPassword": NEW_PASSWORD,
                "confirmPassword": NEW_PASSWORD,
            }))
            .send()
            .await?;
        Ok(response.status.as_u16())
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

register_scenario!(ChangePasswordUnauthorizedScenario);
