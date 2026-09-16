//! 媒体删除:`DELETE /media`。

use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::EntityTrait;
use serde_json::json;
use types::media::media as media_entity;
use types::media::media::MediaId;

use crate::context::Context;

use super::{Session, session, unique_png, unique_tag};

/// 删除前置:登录 + 上传一张唯一图片(得到可删除的目标)。
/// id 以 i64 保存, 便于 `Default`(强类型 ID 不实现 `Default`)。
#[derive(Default)]
pub struct DeleteSetup {
    pub session: Session,
    pub media_id: i64,
    pub file_id: String,
}

/// 删除他人媒体应被忽略, 这里验证"删除自己的媒体"闭环: 库记录与 S3 对象同时消失。
#[derive(Default)]
pub struct DeleteMediaScenario;

impl Scenario for DeleteMediaScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = DeleteSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let view = super::upload(ctx, &session, "e2e.png", unique_png(&unique_tag(task)))
            .await?
            .data;
        let row = media_entity::Entity::find_by_id(view.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .ok_or_else(super::seed_missing)?;
        Ok(DeleteSetup {
            session,
            media_id: row.id.0,
            file_id: row.file_id,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::DELETE, "/media")
            .header("Authorization", &setup.session.auth_header())
            .json_unwrap(&json!({ "mediaIds": [setup.media_id] }))
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let gone = media_entity::Entity::find_by_id(MediaId(setup.media_id))
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

register_scenario!(DeleteMediaScenario);

/// 空媒体列表: 期望 400(参数校验失败)。
#[derive(Default)]
pub struct DeleteMediasEmptyScenario;

impl Scenario for DeleteMediasEmptyScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        session(ctx, task.index).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::DELETE, "/media")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({ "mediaIds": [] }))
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

register_scenario!(DeleteMediasEmptyScenario);

/// 未携带认证头: 期望 401。
#[derive(Default)]
pub struct DeleteMediasUnauthorizedScenario;

impl Scenario for DeleteMediasUnauthorizedScenario {
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
            .request(reqwest::Method::DELETE, "/media")
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

register_scenario!(DeleteMediasUnauthorizedScenario);
