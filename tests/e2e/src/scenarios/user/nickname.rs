use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types::auth;
use types::user::UserInfo;

use crate::context::Context;

use super::session::{Session, login, user_account};

/// 修改昵称: 已被落库, 且缓存已失效(立即读 `/user/me` 返回新昵称).
#[derive(Default)]
pub struct ChangeNicknameScenario;

impl Scenario for ChangeNicknameScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<String>;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &user_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let new_nickname = format!("uit_nick_{}", task.index);
        ctx.client
            .request(reqwest::Method::PATCH, "/user/nickname")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({ "newNickname": new_nickname }))
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
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
            .is_some_and(|u| u.nickname == output.data);

        // 缓存已失效: 紧接着读取个人信息应返回新昵称
        let me: SucR<UserInfo> = ctx
            .client
            .request(reqwest::Method::GET, "/user/me")
            .header("Authorization", &setup.auth_header())
            .send()
            .await?
            .json()
            .await?;

        Ok(db_ok && me.data.nickname == output.data)
    }
}

register_scenario!(
    ChangeNicknameScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 昵称含非法字符: 期望 400.
#[derive(Default)]
pub struct ChangeNicknameInvalidScenario;

impl Scenario for ChangeNicknameInvalidScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &user_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::PATCH, "/user/nickname")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({ "newNickname": "test<script>" }))
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
        Ok(output.code == 400)
    }
}

register_scenario!(
    ChangeNicknameInvalidScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct ChangeNicknameUnauthorizedScenario;

impl Scenario for ChangeNicknameUnauthorizedScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::PATCH, "/user/nickname")
            .json_unwrap(&json!({ "newNickname": "whatever" }))
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

register_scenario!(
    ChangeNicknameUnauthorizedScenario,
    mode = memseek_test::RunMode::Times(32)
);
