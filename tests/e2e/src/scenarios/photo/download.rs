//! 图片下载:`GET /photo/{token}`(公开路由)。

use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::Scenario,
};
use types::photo::dto::photo::PhotoView;

use crate::context::Context;

use super::{Session, session, unique_png, unique_tag};

/// 下载前置:登录 + 上传唯一图片, 记录原图 token 与字节。
#[derive(Default)]
pub struct DownloadSetup {
    pub bytes: Vec<u8>,
    pub original_token: String,
}

/// 通过原图 token 下载: 服务端返回的字节与上传字节一致。
#[derive(Default)]
pub struct DownloadOriginalScenario;

impl Scenario for DownloadOriginalScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = Vec<u8>;

    type Setup = DownloadSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session: Session = session(ctx, task.index).await?;
        let bytes = unique_png(&unique_tag(task));
        let view: PhotoView = super::upload(ctx, &session, "e2e.png", bytes.clone())
            .await?
            .data;
        Ok(DownloadSetup {
            bytes,
            original_token: view.original_token.unwrap_or_default(),
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let resp = ctx
            .client
            .request(
                reqwest::Method::GET,
                &format!("/photo/{}", setup.original_token),
            )
            .send_checked()
            .await?;
        Ok(resp.bytes().await?.to_vec())
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(!setup.original_token.is_empty() && output.as_slice() == setup.bytes.as_slice())
    }
}

register_scenario!(DownloadOriginalScenario);

/// 非法 token: 期望非 2xx(token 解密失败)。
#[derive(Default)]
pub struct DownloadInvalidTokenScenario;

impl Scenario for DownloadInvalidTokenScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(ctx
            .client
            .request(reqwest::Method::GET, "/photo/not_a_valid_image_token")
            .send()
            .await?
            .status()
            .as_u16())
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
