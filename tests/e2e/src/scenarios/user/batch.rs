use memory_seek_api::{Method, api};
use memseek_test::{TaskIndex, register_scenario, scenario::Scenario};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_identity::auth;
use types_identity::auth::user::UserId;
use types_identity::user::{GetUserInfoBatchParam, UserBriefView, UserIds};

use crate::context::Context;
use crate::error::E2eError;

use super::session::{Session, login, user_account};

/// 批量获取用户摘要: 命中项摘要与库中一致, 未找到项返回 `None`.
#[derive(Default)]
pub struct GetUserInfoBatchScenario;

impl Scenario for GetUserInfoBatchScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = Vec<Option<UserBriefView>>;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &user_account(task.index)).await?)
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 取 3 个确定存在的目标用户 id(只读专用池 uit_me, 无人并发写昵称/头像),
        // 外加一个必然不存在的 id
        let targets = auth::user::Entity::find()
            .filter(auth::user::Column::Username.is_in(["uit_me_1", "uit_me_2", "uit_me_3"]))
            .all(&ctx.db)
            .await
            .unwrap();
        let mut ids: Vec<UserId> = targets.iter().map(|u| u.id).collect();
        ids.sort_unstable();
        ids.push(UserId(i64::MAX));

        let param = GetUserInfoBatchParam {
            user_ids: UserIds::new(ids).expect("batch ids 应合法(非空且不超上限)"),
        };
        Ok(api::user::get_user_info_batch(setup.client(), &param).await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let items = output;
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
                Some(token) => match token.decrypt() {
                    Ok(decrypted) => {
                        decrypted.viewer_id == setup.user_id
                            && user.avatar_file_id.as_deref() == Some(decrypted.file_id.as_str())
                    }
                    Err(_) => false,
                },
                None => user.avatar_file_id.is_none(),
            };
            if !avatar_ok {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

register_scenario!(GetUserInfoBatchScenario);

/// 空 id 列表: 期望 400.
#[derive(Default)]
pub struct GetUserInfoBatchEmptyScenario;

impl Scenario for GetUserInfoBatchEmptyScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &user_account(task.index)).await?)
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 空列表无法构造(类型化 `UserIds` 在客户端即拒绝), 故走 raw 直接发非法载荷
        let client = ctx.api_client();
        let auth = setup.auth_header();
        let response = client
            .raw(Method::POST, "/user/batch")
            .header("Authorization", auth)
            .json(&serde_json::json!({ "userIds": [] }))
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
        Ok(*output == 400)
    }
}

register_scenario!(GetUserInfoBatchEmptyScenario);

/// 超过 1024 个 id: 期望 400.
#[derive(Default)]
pub struct GetUserInfoBatchTooManyScenario;

impl Scenario for GetUserInfoBatchTooManyScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(login(ctx, &user_account(task.index)).await?)
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let ids: Vec<i64> = (0..1025).collect();
        let client = ctx.api_client();
        let auth = setup.auth_header();
        let response = client
            .raw(Method::POST, "/user/batch")
            .header("Authorization", auth)
            .json(&serde_json::json!({ "userIds": ids }))
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
        Ok(*output == 400)
    }
}

register_scenario!(GetUserInfoBatchTooManyScenario);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct GetUserInfoBatchUnauthorizedScenario;

impl Scenario for GetUserInfoBatchUnauthorizedScenario {
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
            .raw(Method::POST, "/user/batch")
            .no_auth()
            .json(&serde_json::json!({ "userIds": [1] }))
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

register_scenario!(GetUserInfoBatchUnauthorizedScenario);
