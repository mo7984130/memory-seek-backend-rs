//! 照片删除:`DELETE /photo`。

use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::EntityTrait;
use serde_json::json;
use types::photo::photo as photo_entity;
use types::photo::photo::PhotoId;

use crate::context::Context;

use super::{Session, session, unique_png, unique_tag};

/// 删除前置:登录 + 上传一张唯一图片(得到可删除的目标)。
/// id 以 i64 保存, 便于 `Default`(强类型 ID 不实现 `Default`)。
#[derive(Default)]
pub struct DeleteSetup {
    pub session: Session,
    pub photo_id: i64,
    pub file_id: String,
}

/// 删除他人照片应被忽略, 这里验证"删除自己的照片"闭环: 库记录与 S3 对象同时消失。
#[derive(Default)]
pub struct DeletePhotoScenario;

impl Scenario for DeletePhotoScenario {
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
        let row = photo_entity::Entity::find_by_id(view.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .ok_or_else(super::seed_missing)?;
        Ok(DeleteSetup {
            session,
            photo_id: row.id.0,
            file_id: row.file_id,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::DELETE, "/photo")
            .header("Authorization", &setup.session.auth_header())
            .json_unwrap(&json!({ "photoIds": [setup.photo_id] }))
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
        let gone = photo_entity::Entity::find_by_id(PhotoId(setup.photo_id))
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

register_scenario!(DeletePhotoScenario);

/// 空照片列表: 期望 400(参数校验失败)。
#[derive(Default)]
pub struct DeletePhotosEmptyScenario;

impl Scenario for DeletePhotosEmptyScenario {
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
            .request(reqwest::Method::DELETE, "/photo")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({ "photoIds": [] }))
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

register_scenario!(DeletePhotosEmptyScenario);

/// 未携带认证头: 期望 401。
#[derive(Default)]
pub struct DeletePhotosUnauthorizedScenario;

impl Scenario for DeletePhotosUnauthorizedScenario {
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
            .request(reqwest::Method::DELETE, "/photo")
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

register_scenario!(DeletePhotosUnauthorizedScenario);
