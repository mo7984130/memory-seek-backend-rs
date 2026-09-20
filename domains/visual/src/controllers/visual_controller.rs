use std::sync::Arc;

use axum::{
    Extension, Router,
    body::Body,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::Response,
    routing::{get, post},
};
use bytes::Bytes;
use common::error::{AppError, ContextualError, contextual::ext::OptionExt};
use common::{
    Result,
    axum::{
        R,
        controller_router::ControllerRouter,
        ext::ToROkExt,
        extractors::{ValidatedJson, ValidatedPath, ValidatedQuery},
    },
    time::DateTime,
    types::CursorPage,
};
use types::visual::{
    VisualToken,
    dto::visual::{VisualCursorParam, VisualView},
    models::{DeleteVisualsParam, ExistsByHashBatchParam},
    visual::VisualId,
};
use types::{auth::user::UserId, cursor::TimeIdCursor};

use crate::{
    services::visual_service::{ImageDownloadData, VisualService},
    state::VisualState,
};

/// multipart 中提取出的文件字段数据.
struct UploadedFile {
    file_name: String,
    file_data: Bytes,
}

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
            .route("/check-existence", post(Self::hashes_exist))
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
        let (uploaded, created_at) = Self::collect_upload_parts(&mut multipart).await?;

        let uploaded = uploaded.ok_or_warn(
            "upload_file_not_found",
            "未找到上传文件",
            AppError::bad_request("未找到上传文件"),
        )?;

        let req = types::visual::models::UploadVisualParam {
            file_name: uploaded.file_name,
            created_at,
        };
        let visual =
            VisualService::upload_visual(Arc::clone(&state), user_id, uploaded.file_data, req)
                .await?;

        Ok(visual).to_r_ok()
    }

    /// 解析 multipart 表单: 提取唯一文件字段与可选的 `created_at` 文本字段.
    ///
    /// `Field` 借用 `Multipart`, 故在循环内立即读取并提取数据, 不跨迭代持有字段.
    async fn collect_upload_parts(
        multipart: &mut Multipart,
    ) -> Result<(Option<UploadedFile>, Option<DateTime>)> {
        let mut uploaded: Option<UploadedFile> = None;
        let mut created_at: Option<DateTime> = None;

        while let Some(field) = multipart.next_field().await.map_err(|error| {
            ContextualError::warn(
                "invalid_mutipart",
                "无效的表单数据",
                error,
                AppError::bad_request("无效的表单数据"),
            )
            .emit()
        })? {
            if field.file_name().is_some() {
                if uploaded.is_some() {
                    return Err(ContextualError::warn_without_source(
                        "upload_multi_file",
                        "一次只能上传一个文件",
                        AppError::bad_request("一次只能上传一个文件"),
                    )
                    .emit());
                }
                let file_name = field
                    .file_name()
                    .ok_or_warn(
                        "upload_file_name_not_found",
                        "未找到文件名",
                        AppError::bad_request("未找到文件名"),
                    )?
                    .to_string();
                let file_data = field.bytes().await.map_err(|error| {
                    ContextualError::error(
                        "read_file_err",
                        "读取文件失败",
                        error,
                        AppError::InternalServerError,
                    )
                    .emit()
                })?;
                uploaded = Some(UploadedFile {
                    file_name,
                    file_data,
                });
            } else if field.name() == Some("created_at") {
                let raw = field.text().await.map_err(|error| {
                    ContextualError::warn(
                        "read_created_at_err",
                        "读取 created_at 失败",
                        error,
                        AppError::bad_request("created_at 参数无效"),
                    )
                    .emit()
                })?;
                created_at = Some(raw.parse::<DateTime>().map_err(|error| {
                    ContextualError::warn(
                        "invalid_created_at",
                        "created_at 格式无效，需为 RFC3339 时间",
                        error,
                        AppError::bad_request("created_at 格式无效"),
                    )
                    .emit()
                })?);
            }
        }

        Ok((uploaded, created_at))
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

    /// 批量检查影像哈希值是否已存在.
    async fn hashes_exist(
        State(state): State<Arc<VisualState>>,
        ValidatedJson(req): ValidatedJson<ExistsByHashBatchParam>,
    ) -> Result<R<Vec<bool>>> {
        VisualService::exists_by_hash_batch(&state, req)
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
