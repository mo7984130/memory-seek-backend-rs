use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex, ctxlibs::http_client::HttpError, register_scenario, scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types::auth;
use types::user::UserBriefView;

use crate::context::Context;

use super::session::{Session, login, user_account};

/// 批量获取用户摘要: 命中项摘要与库中一致, 未找到项返回 `None`.
#[derive(Default)]
pub struct GetUserInfoBatchScenario;

impl Scenario for GetUserInfoBatchScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<Vec<Option<UserBriefView>>>;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &user_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 取 3 个确定存在的目标用户 id(通用池), 外加一个必然不存在的 id
        let targets = auth::user::Entity::find()
            .filter(auth::user::Column::Username.is_in(["uit_user_1", "uit_user_2", "uit_user_3"]))
            .all(&ctx.db)
            .await
            .unwrap();
        let mut ids: Vec<i64> = targets.iter().map(|u| u.id.0).collect();
        ids.sort_unstable();
        ids.push(i64::MAX);

        ctx.client
            .request(reqwest::Method::POST, "/user/batch")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({ "userIds": ids }))
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
        let items = &output.data;
        // 3 个命中 + 1 个未命中
        if items.len() != 4 || items.iter().filter(|item| item.is_none()).count() != 1 {
            return Ok(false);
        }

        for view in items.iter().flatten() {
            let user = auth::user::Entity::find()
                .filter(auth::user::Column::Id.eq(view.user_id))
                .one(&ctx.db)
                .await
                .unwrap();
            let Some(user) = user else {
                return Ok(false);
            };
            if user.nickname != view.nickname {
                return Ok(false);
            }

            // 头像 token 的 viewer 应为请求者, 且与库中头像一致
            let avatar_ok = match &view.avatar_token {
                Some(token) => {
                    token.0.viewer_id == setup.user_id
                        && user.avatar_file_id.as_deref() == Some(token.0.file_id.as_str())
                }
                None => user.avatar_file_id.is_none(),
            };
            if !avatar_ok {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

register_scenario!(
    GetUserInfoBatchScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 空 id 列表: 期望 400.
#[derive(Default)]
pub struct GetUserInfoBatchEmptyScenario;

impl Scenario for GetUserInfoBatchEmptyScenario {
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
            .request(reqwest::Method::POST, "/user/batch")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({ "userIds": [] }))
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
    GetUserInfoBatchEmptyScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 超过 1024 个 id: 期望 400.
#[derive(Default)]
pub struct GetUserInfoBatchTooManyScenario;

impl Scenario for GetUserInfoBatchTooManyScenario {
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
        let ids: Vec<i64> = (0..1025).collect();
        ctx.client
            .request(reqwest::Method::POST, "/user/batch")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({ "userIds": ids }))
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
    GetUserInfoBatchTooManyScenario,
    mode = memseek_test::RunMode::Times(32)
);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct GetUserInfoBatchUnauthorizedScenario;

impl Scenario for GetUserInfoBatchUnauthorizedScenario {
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
            .post_raw("/user/batch", json!({ "userIds": [1] }))
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
    GetUserInfoBatchUnauthorizedScenario,
    mode = memseek_test::RunMode::Times(32)
);
