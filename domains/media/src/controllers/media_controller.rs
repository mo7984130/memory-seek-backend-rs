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
use types::media::{
    ImageToken,
    dto::media::{MediaCursorParam, MediaView},
    models::{DeleteMediasParam, ExistsByMd5BatchParam},
    media::MediaId,
};
use types::{auth::user::UserId, cursor::TimeIdCursor};

use crate::{
    services::media_service::{ImageDownloadData, MediaService},
    state::MediaState,
};

pub struct MediaController;

impl ControllerRouter for MediaController {
    type State = MediaState;

    fn protected_routes() -> Router<Arc<MediaState>> {
        Router::new()
            .route(
                "/",
                get(Self::get_medias_cursor)
                    .post(Self::upload)
                    .delete(Self::delete_medias),
            )
            .route("/media/{media_id}", get(Self::get_media_info))
            .route("/check-existence", post(Self::md5s_exist))
    }

    fn public_routes() -> Router<Arc<MediaState>> {
        Router::new().route("/{token}", get(Self::get_image))
    }
}

impl MediaController {
    /// 接收 multipart 图片, 完成校验, 存储并记录上传行为.
    async fn upload(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        mut multipart: Multipart,
    ) -> Result<R<MediaView>> {
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

        let file_name = field.file_name().unwrap_or("media.jpg").to_string();
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

        let req = types::media::models::UploadMediaParam {
            file_name,
            content_type,
        };
        let media = MediaService::upload_media(Arc::clone(&state), user_id, file_data, req).await?;

        Ok(media).to_r_ok()
    }

    /// 游标获取媒体列表.
    async fn get_medias_cursor(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<MediaCursorParam>,
    ) -> Result<R<CursorPage<MediaView, TimeIdCursor<MediaId>>>> {
        MediaService::get_media_cursor_page(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 获取单张媒体信息
    async fn get_media_info(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedPath(media_id): ValidatedPath<MediaId>,
    ) -> Result<R<MediaView>> {
        MediaService::get_media_info(&state, user_id, media_id)
            .await
            .to_r_ok()
    }

    /// 批量检查图片 MD5 是否已存在.
    async fn md5s_exist(
        State(state): State<Arc<MediaState>>,
        ValidatedJson(req): ValidatedJson<ExistsByMd5BatchParam>,
    ) -> Result<R<Vec<bool>>> {
        MediaService::exists_by_md5_batch(&state, req)
            .await
            .to_r_ok()
    }

    /// 解密图片访问令牌并返回原图或处理后的图片流.
    async fn get_image(
        State(state): State<Arc<MediaState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>> {
        let image_token: ImageToken = ImageToken::decrypt(&token)?;

        let data = MediaService::download_image(&state, image_token).await?;

        let resp = match data {
            ImageDownloadData::Processed(bytes) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/webp")
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

    /// 删除当前用户指定的媒体及其对象存储文件.
    async fn delete_medias(
        State(state): State<Arc<MediaState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<DeleteMediasParam>,
    ) -> Result<R<()>> {
        MediaService::delete_medias(state, user_id, req)
            .await
            .to_r_ok()?;

        Ok(()).to_r_ok()
    }
}
