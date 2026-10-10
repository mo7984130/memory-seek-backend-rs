use memory_seek_api::{Method, api};
use memseek_test::{TaskIndex, register_scenario, scenario::Scenario};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_identity::auth;
use types_identity::user::ChangeNicknameParam;

use crate::context::Context;
use crate::error::{E2eError, status};

use super::session::{Session, login, user_account};

/// 修改昵称: 已被落库, 且缓存已失效(立即读 `/user/me` 返回新昵称).
#[derive(Default)]
pub struct ChangeNicknameScenario;

impl Scenario for ChangeNicknameScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = String;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &user_account(task.index)).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let param = ChangeNicknameParam {
            new_nickname: format!("uit_nick_{}", task.index),
        };
        Ok(api::user::change_nickname(setup.client(), &param).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let db_ok = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.user_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some_and(|u| u.nickname == *output);

        // 缓存已失效: 紧接着读取个人信息应返回新昵称
        let me = api::user::get_me(setup.client()).await?;

        Ok(db_ok && me.nickname == *output)
    }
}

register_scenario!(ChangeNicknameScenario);

/// 昵称含非法字符: 期望 400.
#[derive(Default)]
pub struct ChangeNicknameInvalidScenario;

impl Scenario for ChangeNicknameInvalidScenario {
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
        let param = ChangeNicknameParam {
            new_nickname: "test<script>".to_owned(),
        };
        status(api::user::change_nickname(setup.client(), &param).await)
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

register_scenario!(ChangeNicknameInvalidScenario);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct ChangeNicknameUnauthorizedScenario;

impl Scenario for ChangeNicknameUnauthorizedScenario {
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
            .raw(Method::PATCH, "/user/nickname")
            .no_auth()
            .json(&serde_json::json!({ "newNickname": "whatever" }))
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

register_scenario!(ChangeNicknameUnauthorizedScenario);
