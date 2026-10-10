use memory_seek_api::{Method, api};
use memseek_test::{TaskIndex, register_scenario, scenario::Scenario};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_identity::auth;
use types_identity::user::UserInfo;

use crate::context::Context;
use crate::error::E2eError;

use super::session::{Session, login, me_account};

/// 获取当前用户信息: 响应必须与库中记录一致.
#[derive(Default)]
pub struct MeScenario;

impl Scenario for MeScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = UserInfo;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &me_account(task.index)).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::user::get_me(setup.client()).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let db_user = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.user_id))
            .one(&ctx.db)
            .await
            .unwrap();

        let Some(user) = db_user else {
            return Ok(false);
        };

        let fields_ok = user.username == output.username
            && user.email == output.email
            && user.nickname == output.nickname;
        // 头像 token 须与库中头像一致(含 viewer 绑定); 无头像时响应也应为 None
        let avatar_ok = match &output.avatar_token {
            Some(token) => match token.decrypt() {
                Ok(decrypted) => {
                    decrypted.viewer_id == setup.user_id
                        && user.avatar_file_id.as_deref() == Some(decrypted.file_id.as_str())
                }
                Err(_) => false,
            },
            None => user.avatar_file_id.is_none(),
        };

        Ok(output.id == setup.user_id && fields_ok && avatar_ok)
    }
}

register_scenario!(MeScenario);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct MeUnauthorizedScenario;

impl Scenario for MeUnauthorizedScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 无鉴权走低层 raw:类型化接口在客户端就拒绝未登录, 观测不到服务端的 401
        let client = ctx.api_client();
        let response = client.raw(Method::GET, "/user/me").no_auth().send().await?;
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

register_scenario!(MeUnauthorizedScenario);

/// 伪造 access_token(用户存在但 token 不匹配): 期望 401.
#[derive(Default)]
pub struct MeForgedTokenScenario;

impl Scenario for MeForgedTokenScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let client = ctx.api_client();
        let authorization = format!("Bearer {} forged_token", task.index + 1);
        let response = client
            .raw(Method::GET, "/user/me")
            .header("Authorization", authorization)
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

register_scenario!(MeForgedTokenScenario);
