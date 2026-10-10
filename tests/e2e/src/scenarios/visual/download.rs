//! 影像下载:`GET /visual/{token}`(公开路由)。

use memory_seek_api::{VisualIds, api};
use memseek_test::{TaskIndex, register_scenario, scenario::Scenario};
use types_visual::visual::VisualId;

use crate::context::Context;
use crate::error::E2eError;

use super::{Session, session, unique_png, unique_tag};

/// 下载前置:登录 + 上传唯一影像, 记录原图 token 与字节。
/// `visual_id` 供收尾删除(长跑下每任务每轮上传, 不回收会持续增长)。
pub struct DownloadSetup {
    pub bytes: Vec<u8>,
    pub original_token: String,
    pub thumbnail_token: String,
    pub visual_id: VisualId,
    pub session: Session,
}

impl Default for DownloadSetup {
    fn default() -> Self {
        Self {
            bytes: Vec::new(),
            original_token: String::new(),
            thumbnail_token: String::new(),
            visual_id: VisualId(0),
            session: Session::default(),
        }
    }
}

/// 收尾共用: 删除 setup 阶段上传的影像。
async fn teardown(setup: &DownloadSetup) -> Result<(), E2eError> {
    crate::cleanup::idempotent(
        api::visual::delete_visuals(
            setup.session.client(),
            VisualIds::new(vec![setup.visual_id]).expect("单元素影像列表恒合法"),
        )
        .await,
    )
}

/// 通过原图 token 下载: 服务端返回的字节与上传字节一致。
#[derive(Default)]
pub struct DownloadOriginalScenario;

impl Scenario for DownloadOriginalScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = Vec<u8>;

    type Setup = DownloadSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session: Session = session(ctx, task.index).await?;
        let bytes = unique_png(&unique_tag(task));
        let view = super::upload(&session, bytes.clone()).await?;
        Ok(DownloadSetup {
            bytes,
            original_token: view.original_token.unwrap_or_default(),
            thumbnail_token: view.thumbnail_token.unwrap_or_default(),
            visual_id: view.id,
            session,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let response = api::visual::download(&ctx.api_client(), &setup.original_token).await?;
        if !response.is_success() {
            return Err(E2eError::internal(format!(
                "下载原图失败: HTTP {}",
                response.status.as_u16()
            )));
        }
        Ok(response.body)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(!setup.original_token.is_empty() && output.as_slice() == setup.bytes.as_slice())
    }

    /// 收尾: 删除 setup 阶段上传的影像(每任务一张, 默认 Task 粒度)
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        teardown(setup).await
    }
}

register_scenario!(DownloadOriginalScenario);

/// 图片缩略图: 经图片处理后端(本地/OSS)产出 WebP。
#[derive(Default)]
pub struct DownloadImageThumbnailScenario;

impl Scenario for DownloadImageThumbnailScenario {
    type Ctx = Context;

    type Error = E2eError;

    /// (状态码, content-type, 字节)
    type Output = (u16, Option<String>, Vec<u8>);

    type Setup = DownloadSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session: Session = session(ctx, task.index).await?;
        let bytes = unique_png(&unique_tag(task));
        let view = super::upload(&session, bytes.clone()).await?;
        Ok(DownloadSetup {
            bytes,
            original_token: view.original_token.unwrap_or_default(),
            thumbnail_token: view.thumbnail_token.unwrap_or_default(),
            visual_id: view.id,
            session,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let response = api::visual::download(&ctx.api_client(), &setup.thumbnail_token).await?;
        let status = response.status.as_u16();
        let content_type = response
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        Ok((status, content_type, response.body))
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let (status, content_type, bytes) = output;
        // 缩略图应为 WebP(RIFF 容器 + 偏移 8 处 "WEBP")
        let is_webp = bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP";
        Ok(!setup.thumbnail_token.is_empty()
            && *status == 200
            && content_type.as_deref() == Some("image/webp")
            && is_webp)
    }

    /// 收尾: 删除 setup 阶段上传的影像。
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        teardown(setup).await
    }
}

register_scenario!(DownloadImageThumbnailScenario);

/// 非法 token: 期望非 2xx(token 解密失败)。
#[derive(Default)]
pub struct DownloadInvalidTokenScenario;

impl Scenario for DownloadInvalidTokenScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let response = api::visual::download(&ctx.api_client(), "not_a_valid_image_token").await?;
        Ok(response.status.as_u16())
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output >= 400)
    }
}

register_scenario!(DownloadInvalidTokenScenario);
