//! 图片人脸裁剪(`Crop`): 构造 crop token 验证裁剪处理后端产出 WebP。
//!
//! crop token 本身携带 bbox 与源尺寸, 无需人脸数据; 直接对已上传影像构造即可。

use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types_visual::dto::visual::VisualView;
use types_visual::visual as visual_entity;
use types_visual::{FaceBBox, ImageDimensions, VisualToken};

use crate::context::Context;

use super::{Session, session, unique_png, unique_tag};

/// setup 阶段内部失败(取记录 / 加密失败)时返回的错误。
fn setup_error() -> HttpError {
    HttpError::Status {
        status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        method: reqwest::Method::GET,
        url: "crop://setup".to_string(),
        request_body: None,
        response_body: None,
    }
}

/// 裁剪下载前置: 登录 + 上传唯一影像 + 构造 crop token。
#[derive(Default)]
pub struct CropSetup {
    pub crop_token: String,
    pub visual_id: i64,
    pub auth_header: String,
}

/// 人脸裁剪: 经裁剪处理后端(本地/OSS)产出 WebP。
#[derive(Default)]
pub struct DownloadImageCropScenario;

impl Scenario for DownloadImageCropScenario {
    type Ctx = Context;

    type Error = HttpError;

    /// (状态码, content-type, 字节)
    type Output = (u16, Option<String>, Vec<u8>);

    type Setup = CropSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session: Session = session(ctx, task.index).await?;
        let bytes = unique_png(&unique_tag(task));
        let view: VisualView = super::upload(ctx, &session, bytes).await?.data;

        // 取回 file_id 与尺寸, 用于构造携带 bbox 的 crop token
        let row = visual_entity::Entity::find()
            .filter(visual_entity::Column::Id.eq(view.id))
            .one(&ctx.db)
            .await
            .map_err(|_| setup_error())?
            .ok_or_else(setup_error)?;

        let crop_token = VisualToken::crop(
            session.user_id,
            row.file_id.clone(),
            FaceBBox {
                x1: 0.0,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
            ImageDimensions {
                width: row.width as u32,
                height: row.height as u32,
            },
        )
        .encrypt()
        .map_err(|_| setup_error())?;

        Ok(CropSetup {
            crop_token,
            visual_id: view.id.0,
            auth_header: session.auth_header(),
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
                &format!("/visual/{}", setup.crop_token),
            )
            .send()
            .await?;
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let bytes = resp.bytes().await?.to_vec();
        Ok((status, content_type, bytes))
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let (status, content_type, bytes) = output;
        // 裁剪产物应为 WebP(RIFF 容器 + 偏移 8 处 "WEBP")
        let is_webp = bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP";
        Ok(!setup.crop_token.is_empty()
            && *status == 200
            && content_type.as_deref() == Some("image/webp")
            && is_webp)
    }

    /// 收尾: 删除 setup 阶段上传的影像。
    async fn teardown(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        crate::cleanup::delete(
            ctx,
            "/visual",
            Some(&setup.auth_header),
            Some(json!({ "visualIds": [setup.visual_id] })),
        )
        .await
    }
}

register_scenario!(DownloadImageCropScenario);
