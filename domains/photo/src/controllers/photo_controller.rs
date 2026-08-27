use std::sync::Arc;

use axum::{
    Extension, Router,
    body::Body,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::Response,
    routing::{get, post},
};
use common::{
    Result,
    axum::{
        R,
        controller_router::ControllerRouter,
        ext::ToROkExt,
        extractors::{ValidatedJson, ValidatedQuery},
    },
    types::CursorPage,
};
use common::{
    error::{AppError, ContextualError, contextual::ext::OptionExt},
    utils::token_cipher,
};
use types::photo::{
    ImageToken,
    dto::photo::{PhotoCursorParam, PhotoView},
    models::{DeletePhotosParam, ExistsByMd5BatchParam},
    photo::PhotoId,
};
use types::{auth::user::UserId, cursor::TimeIdCursor};

use crate::{
    services::photo_service::{ImageDownloadData, PhotoService},
    state::PhotoState,
};

pub struct PhotoController;

impl ControllerRouter for PhotoController {
    type State = PhotoState;

    fn protected_routes() -> Router<Arc<PhotoState>> {
        Router::new()
            .route(
                "/",
                get(Self::get_photos_cursor)
                    .post(Self::upload)
                    .delete(Self::delete_photos),
            )
            .route("/check-existence", post(Self::md5s_exist))
    }

    fn public_routes() -> Router<Arc<PhotoState>> {
        Router::new().route("/{token}", get(Self::get_image))
    }
}

impl PhotoController {
    /// 接收 multipart 图片, 完成校验, 存储并记录上传行为.
    async fn upload(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        mut multipart: Multipart,
    ) -> Result<R<PhotoView>> {
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

        let file_name = field.file_name().unwrap_or("photo.jpg").to_string();
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

        let req = types::photo::models::UploadPhotoParam {
            file_name,
            content_type,
        };
        let photo = PhotoService::upload_photo(Arc::clone(&state), user_id, file_data, req).await?;

        Ok(photo).to_r_ok()
    }

    /// 游标获取照片列表.
    async fn get_photos_cursor(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<PhotoCursorParam>,
    ) -> Result<R<CursorPage<PhotoView, TimeIdCursor<PhotoId>>>> {
        PhotoService::get_photo_cursor_page(&state, user_id, req)
            .await
            .to_r_ok()
    }

    /// 批量检查图片 MD5 是否已存在.
    async fn md5s_exist(
        State(state): State<Arc<PhotoState>>,
        ValidatedJson(req): ValidatedJson<ExistsByMd5BatchParam>,
    ) -> Result<R<Vec<bool>>> {
        PhotoService::exists_by_md5_batch(&state, req)
            .await
            .to_r_ok()
    }

    /// 解密图片访问令牌并返回原图或处理后的图片流.
    async fn get_image(
        State(state): State<Arc<PhotoState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>> {
        let image_token: ImageToken = token_cipher()
            .decrypt(&token)
            .map_err(ContextualError::emit)?;

        let data = PhotoService::download_image(&state, image_token).await?;

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

    /// 删除当前用户指定的照片及其对象存储文件.
    async fn delete_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(req): ValidatedJson<DeletePhotosParam>,
    ) -> Result<R<()>> {
        PhotoService::delete_photos(state, user_id, req)
            .await
            .to_r_ok()?;

        Ok(()).to_r_ok()
    }
}
