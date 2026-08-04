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
    ext::{ResultErrExt, ResultRExt},
    extractors::{ValidatedJson, ValidatedQuery},
    models::{CursorPage, TimeIdCursor},
    traits::controller::ControllerRouter,
};
use common::{ext::OptionExt, r::R};
use types::auth::user::UserId;
use types::photo::{
    ImageToken,
    dto::photo::{PhotoCursorParam, PhotoView},
    models::{DeletePhotosParam, ExistsByMd5BatchParam},
    photo::PhotoId,
};

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
    async fn upload(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        mut multipart: Multipart,
    ) -> Result<R<PhotoView>> {
        let field = multipart
            .next_field()
            .await
            .trace_warn_bad_request("invalid_mutipart", "无效的表单数据", "无效的表单数据")?
            .ok_or_warn_bad_request("upload_file_not_found", "未找到上传文件", "未找到上传文件")?;

        let file_name = field.file_name().unwrap_or("photo.jpg").to_string();
        let content_type = field.content_type().unwrap_or("image/jpg").to_string();
        let file_data = field
            .bytes()
            .await
            .trace_internal_err("read_file_err", "读取文件失败")?;

        let param = types::photo::models::UploadPhotoParam {
            file_name,
            content_type,
            created_at: None,
        };
        PhotoService::upload_photo(&state, user_id, file_data, param)
            .await
            .to_r_ok()
    }

    async fn get_photos_cursor(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(query): ValidatedQuery<PhotoCursorParam>,
    ) -> Result<R<CursorPage<PhotoView, String>>> {
        let PhotoCursorParam {
            cursor,
            size,
            direction,
            anchor_time,
            ..
        } = query;

        let cursor = cursor.map(TimeIdCursor::<PhotoId>::decode).transpose()?;

        PhotoService::get_photo_cursor_page(&state, user_id, cursor, size, direction, anchor_time)
            .await
            .to_r_ok()
    }

    async fn md5s_exist(
        State(state): State<Arc<PhotoState>>,
        ValidatedJson(data): ValidatedJson<ExistsByMd5BatchParam>,
    ) -> Result<R<Vec<bool>>> {
        PhotoService::exists_by_md5_batch(&state, data)
            .await
            .to_r_ok()
    }

    async fn get_image(
        State(state): State<Arc<PhotoState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>> {
        let image_token: ImageToken = state.token_cipher.decrypt(&token)?;

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

    async fn delete_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(data): ValidatedJson<DeletePhotosParam>,
    ) -> Result<R<()>> {
        PhotoService::delete_photos(&state, user_id, data)
            .await
            .to_r_ok()
    }
}
