use std::sync::Arc;

use axum::{
    Extension, Router,
    body::Body,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::Response,
    routing::{get, post},
};
use common::error::{AppError, ContextualError, contextual::ext::OptionExt};
use common::{
    Result,
    axum::{
        R,
        controller_router::ControllerRouter,
        ext::ToROkExt,
        extractors::{ValidatedJson, ValidatedPath, ValidatedQuery},
    },
    types::CursorPage,
};
use types::visual::{
    VisualToken,
    dto::visual::{VisualCursorParam, VisualView},
    models::{DeleteVisualsParam, ExistsByMd5BatchParam},
    visual::VisualId,
};
use types::{auth::user::UserId, cursor::TimeIdCursor};

use crate::{
    services::visual_service::{ImageDownloadData, VisualService},
    state::VisualState,
};

pub struct VisualController;

impl ControllerRouter for VisualController {
    type State = VisualState;

    fn protected_routes() -> Router<Arc<VisualState>> {
        Router::new()
            .route(
                "/",
                get(Self::get_visuals_cursor)
                    .post(Self::upload)
                    .delete(Self::delete_visuals),
            )
            .route("/visual/{visual_id}", get(Self::get_visual_info))
            .route("/check-existence", post(Self::md5s_exist))
    }

    fn public_routes() -> Router<Arc<VisualState>> {
        Router::new().route("/{token}", get(Self::get_visual))
    }
}

impl VisualController {
    /// 接收 multipart 影像, 完成校验, 存储并记录上传行为.
    async fn upload(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        mut multipart: Multipart,
    ) -> Result<R<VisualView>> {
        let field = multipart
            .next_field()
            .await
            .map_err(|error| {
                ContextualError::warn(
                    "invalid_mutipart",
                    "无效的表单数据",
                    error,
                    AppError::bad_request("无效的表单数据"),
                )
                .emit()
            })?
            .ok_or_warn(
                "upload_file_not_found",
                "未找到上传文件",
                AppError::bad_request("未找到上传文件"),
            )?;

        let file_name = field.file_name().unwrap_or("visual.jpg").to_string();
        let content_type = field.content_type().unwrap_or("image/jpg").to_string();
        let file_data = field.bytes().await.map_err(|error| {
            ContextualError::error(
                "read_file_err",
                "读取文件失败",
                error,
                AppError::InternalServerError,
            )
            .emit()
        })?;

        let req = types::visual::models::UploadVisualParam {
            file_name,
            content_type,
        };
        let visual =
            VisualService::upload_visual(Arc::clone(&state), user_id, file_data, req).await?;

        Ok(visual).to_r_ok()
    }

    /// 游标获取影像列表.
    async fn get_visuals_cursor(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<VisualCursorParam>,
    ) -> Result<R<CursorPage<VisualView, TimeIdCursor<VisualId>>>> {
        VisualService::get_visual_cursor_page(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 获取单张影像信息
    async fn get_visual_info(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(visual_id): ValidatedPath<VisualId>,
    ) -> Result<R<VisualView>> {
        VisualService::get_visual_info(&state, user_id, visual_id)
            .await
            .to_r_ok()
    }

    /// 批量检查影像 MD5 是否已存在.
    async fn md5s_exist(
        State(state): State<Arc<VisualState>>,
        ValidatedJson(req): ValidatedJson<ExistsByMd5BatchParam>,
    ) -> Result<R<Vec<bool>>> {
        VisualService::exists_by_md5_batch(&state, req)
            .await
            .to_r_ok()
    }

    /// 解密视觉访问令牌并返回原图/原视频或处理后的影像流(图片处理/视频截帧).
    async fn get_visual(
        State(state): State<Arc<VisualState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>> {
        let visual_token: VisualToken = VisualToken::decrypt(&token)?;

        let data = VisualService::download_visual(&state, visual_token).await?;

        let resp = match data {
            ImageDownloadData::Processed {
                bytes,
                content_type,
            } => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CACHE_CONTROL, "public, max-age=604800")
                .body(Body::from(bytes))
                .unwrap(),
            ImageDownloadData::Original {
                stream,
                content_type,
            } => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CACHE_CONTROL, "public, max-age=604800")
                .body(Body::from_stream(stream))
                .unwrap(),
        };

        Ok(resp)
    }

    /// 删除当前用户指定的影像及其对象存储文件.
    async fn delete_visuals(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<DeleteVisualsParam>,
    ) -> Result<R<()>> {
        VisualService::delete_visuals(state, user_id, req)
            .await
            .to_r_ok()?;

        Ok(()).to_r_ok()
    }
}
