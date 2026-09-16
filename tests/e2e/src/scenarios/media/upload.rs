//! 照片上传:`POST /media`。

use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::EntityTrait;
use types::media::dto::media::MediaView;
use types::media::media as media_entity;

use crate::context::Context;

use super::{Session, file_form, md5_hex, session, token_matches, unique_png, unique_tag};

/// 上传前置:登录 + 本次任务唯一图片字节。
#[derive(Default)]
pub struct UploadSetup {
    pub session: Session,
    pub bytes: Vec<u8>,
}

async fn prepare(ctx: &Context, task: &TaskIndex) -> Result<UploadSetup, HttpError> {
    let session = session(ctx, task.index).await?;
    Ok(UploadSetup {
        session,
        bytes: unique_png(&unique_tag(task)),
    })
}

/// 上传成功: 响应字段、库中记录与 S3 对象三者一致, 且三档 token 绑定浏览者。
#[derive(Default)]
pub struct UploadMediaScenario;

impl Scenario for UploadMediaScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<MediaView>;

    type Setup = UploadSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        prepare(ctx, task).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        super::upload(ctx, &setup.session, "e2e.png", setup.bytes.clone()).await
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let view = &output.data;
        let Some(row) = media_entity::Entity::find_by_id(view.id)
            .one(&ctx.db)
            .await
            .unwrap()
        else {
            return Ok(false);
        };

        // 落库字段与请求内容一致
        let db_ok = row.user_id == setup.session.user_id
            && row.name == "e2e.png"
            && row.mime_type == "image/png"
            && row.md5 == md5_hex(&setup.bytes)
            && row.size as u64 == setup.bytes.len() as u64
            && row.width == 1
            && row.height == 1;

        // 响应与库一致
        let view_ok = view.user_id == setup.session.user_id
            && view.name == row.name
            && view.width == 1
            && view.height == 1;

        // 三档 token 均绑定到 file_id 与当前浏览者
        let viewer = setup.session.user_id;
        let tokens_ok = token_matches(view.thumbnail_token.as_ref(), &row.file_id, viewer)
            && token_matches(view.preview_token.as_ref(), &row.file_id, viewer)
            && token_matches(view.original_token.as_ref(), &row.file_id, viewer);

        // S3 对象已落盘且内容与上传字节一致
        let s3_ok = ctx.s3_bytes(&row.file_id).await.as_deref() == Some(setup.bytes.as_slice());

        Ok(db_ok && view_ok && tokens_ok && s3_ok)
    }
}

register_scenario!(UploadMediaScenario);

/// 重复上传同一内容: 期望 400(命中 md5 去重)。
#[derive(Default)]
pub struct UploadDuplicateMediaScenario;

impl Scenario for UploadDuplicateMediaScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = UploadSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let setup = prepare(ctx, task).await?;
        // 先成功上传一次, 为 run 制造 md5 命中
        super::upload(ctx, &setup.session, "e2e.png", setup.bytes.clone()).await?;
        Ok(setup)
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let form = file_form("e2e.png", "image/png", setup.bytes.clone())?;
        ctx.client
            .request(reqwest::Method::POST, "/media")
            .header("Authorization", &setup.session.auth_header())
            .multipart(form)
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

register_scenario!(UploadDuplicateMediaScenario);

/// 非法文件: 期望 400(文件校验失败)。
#[derive(Default)]
pub struct UploadInvalidFileScenario;

impl Scenario for UploadInvalidFileScenario {
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
        let form = file_form("evil.txt", "text/plain", b"not an image".to_vec())?;
        ctx.client
            .request(reqwest::Method::POST, "/media")
            .header("Authorization", &setup.auth_header())
            .multipart(form)
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

register_scenario!(UploadInvalidFileScenario);

/// 未携带认证头: 期望 401。
#[derive(Default)]
pub struct UploadUnauthorizedScenario;

impl Scenario for UploadUnauthorizedScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let form = file_form("e2e.png", "image/png", unique_png(&unique_tag(task)))?;
        ctx.client
            .request(reqwest::Method::POST, "/media")
            .multipart(form)
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

register_scenario!(UploadUnauthorizedScenario);
