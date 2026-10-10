//! 影像删除:`DELETE /visual`。

use memory_seek_api::{Method, VisualIds, api};
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::EntityTrait;
use serde_json::json;
use types_visual::visual as visual_entity;
use types_visual::visual::VisualId;

use crate::context::Context;
use crate::error::E2eError;

use super::{Session, session, unique_png, unique_tag};

/// 删除前置:登录 + 上传一张唯一影像(得到可删除的目标)。
pub struct DeleteSetup {
    pub session: Session,
    pub visual_id: VisualId,
    pub file_id: String,
}

impl Default for DeleteSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            visual_id: VisualId(0),
            file_id: String::new(),
        }
    }
}

/// 删除他人影像应被忽略, 这里验证"删除自己的影像"闭环: 库记录与 S3 对象同时消失。
#[derive(Default)]
pub struct DeleteVisualScenario;

impl Scenario for DeleteVisualScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = DeleteSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let view = super::upload(&session, unique_png(&unique_tag(task))).await?;
        let row = visual_entity::Entity::find_by_id(view.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .ok_or_else(super::seed_missing)?;
        Ok(DeleteSetup {
            session,
            visual_id: row.id,
            file_id: row.file_id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::delete_visuals(
            setup.session.client(),
            VisualIds::new(vec![setup.visual_id]).expect("单元素影像列表恒合法"),
        )
        .await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let gone = visual_entity::Entity::find_by_id(setup.visual_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        if !gone {
            return Ok(false);
        }
        // S3 对象由异步事件消费者删除(最终一致), 需轮询等待消失
        for _ in 0..20 {
            if ctx.s3_missing(&setup.file_id).await {
                return Ok(true);
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Ok(false)
    }
}

register_scenario!(DeleteVisualScenario);

/// 空影像列表: 期望 400(参数校验失败)。
#[derive(Default)]
pub struct DeleteVisualsEmptyScenario;

impl Scenario for DeleteVisualsEmptyScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(session(ctx, task.index).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 空列表无法构造(类型化 `VisualIds` 在客户端即拒绝), 故走 raw 直接发非法载荷
        let response = setup
            .client()
            .raw(Method::DELETE, "/visual")
            .json(&json!({ "visualIds": [] }))
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

register_scenario!(DeleteVisualsEmptyScenario);

/// 未携带认证头: 期望 401。
#[derive(Default)]
pub struct DeleteVisualsUnauthorizedScenario;

impl Scenario for DeleteVisualsUnauthorizedScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let response = ctx
            .api_client()
            .raw(Method::DELETE, "/visual")
            .no_auth()
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

register_scenario!(DeleteVisualsUnauthorizedScenario);
