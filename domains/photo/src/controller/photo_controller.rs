use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::Extension;
use axum::Router;
use common::error::AppError;
use common::r::R;
use common::utils::ResultExt;
use img_url_generator::{decrypt_image_token, ImageToken, ImageTokenType};
use std::sync::Arc;
use tracing::debug;
use crate::middlewares::auth::UserId;
use crate::state::AppState;
use crate::models::photo::{CursorPageVO, Md5Query, PhotoCursorQuery, PhotoVO, TimeRangeVO, UploadWithCreatedAtQuery};
use crate::services::photo_service::PhotoService;

pub struct PhotoController;

impl PhotoController {
    pub fn routes() -> Router<Arc<AppState>> {
        Router::new()
            .route("/upload", post(Self::upload))
            .route("/upload/with-created-at", post(Self::upload_with_created_at))
            .route("/cursor", get(Self::get_photos_cursor))
            .route("/md5-exist", get(Self::md5_exist))
            .route("/time-range", get(Self::get_time_range))
            .route("/{id}", delete(Self::delete_photo))
    }

    pub fn public_routes() -> Router<Arc<AppState>> {
        Router::new()
            .route("/{token}", get(Self::get_image))
            .route("/{token}/thumbnail", get(Self::get_thumbnail))
            .route("/{token}/preview", get(Self::get_preview))
            .route("/{token}/original", get(Self::get_original))
            .route("/{token}/crop", get(Self::get_crop))
    }

    async fn upload(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        mut multipart: Multipart,
    ) -> Result<R<PhotoVO>, AppError> {
        let field = multipart
            .next_field()
            .await
            .map_err(|_| AppError::bad_request("无效的表单数据"))?
            .ok_or_else(|| AppError::bad_request("未找到上传文件"))?;

        let file_name = field.file_name().unwrap_or("photo_entities.jpg").to_string();
        let content_type = field.content_type().unwrap_or("image/jpeg").to_string();
        let file_data = field
            .bytes()
            .await
            .map_internal_err("读取文件失败")?;

        let photo = PhotoService::upload_photo(
            &state.db,
            &state.redis,
            &state.s3_client,
            #[cfg(feature = "face_recognition")]
            &state.face_tx,
            user_id.0,
            file_data,
            file_name,
            content_type,
            None,
            &state.encryption_key,
        )
        .await?;

        Ok(R::ok(photo))
    }

    async fn upload_with_created_at(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Query(query): Query<UploadWithCreatedAtQuery>,
        mut multipart: Multipart,
    ) -> Result<R<PhotoVO>, AppError> {
        if user_id.0 != 1 {
            return Err(AppError::bad_request("只有管理员可以上传指定时间的照片"));
        }

        let field = multipart
            .next_field()
            .await
            .map_err(|_| AppError::bad_request("无效的表单数据"))?
            .ok_or_else(|| AppError::bad_request("未找到上传文件"))?;

        let file_name = field.file_name().unwrap_or("photo_entities.jpg").to_string();
        let content_type = field.content_type().unwrap_or("image/jpeg").to_string();
        let file_data = field
            .bytes()
            .await
            .map_internal_err("读取文件失败")?;

        let photo = PhotoService::upload_photo(
            &state.db,
            &state.redis,
            &state.s3_client,
            #[cfg(feature = "face_recognition")]
            &state.face_tx,
            user_id.0,
            file_data,
            file_name,
            content_type,
            Some(query.created_at),
            &state.encryption_key,
        )
        .await?;

        Ok(R::ok(photo))
    }

    async fn get_photos_cursor(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Query(query): Query<PhotoCursorQuery>,
    ) -> Result<R<CursorPageVO<PhotoVO, String>>, AppError> {
        let result =
            PhotoService::get_photo_cursor_page(&state.db, &state.redis, user_id.0, query, &state.encryption_key)
                .await?;
        Ok(R::ok(result))
    }

    async fn md5_exist(
        State(state): State<Arc<AppState>>,
        Query(params): Query<Md5Query>,
    ) -> Result<R<bool>, AppError> {
        let exists = PhotoService::md5_exists(&state.db, &params.md5).await?;
        Ok(R::ok(exists))
    }

    async fn get_time_range(
        State(state): State<Arc<AppState>>,
    ) -> Result<R<TimeRangeVO>, AppError> {
        let (min, max) = PhotoService::get_time_range(&state.db).await?;
        Ok(R::ok(TimeRangeVO { min, max }))
    }

    async fn delete_photo(
        State(state): State<Arc<AppState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
    ) -> Result<R<()>, AppError> {
        let photo_id: i64 = id.parse().map_err(|_| AppError::bad_request("无效的照片ID"))?;

        PhotoService::delete_photo(
            &state.db,
            #[cfg(feature = "face_recognition")]
            &state.redis,
            &state.s3_client,
            user_id.0,
            photo_id,
        )
        .await?;

        Ok(R::ok(()))
    }

    async fn get_image(
        State(state): State<Arc<AppState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        let image_token: ImageToken = decrypt_image_token(&token, &state.encryption_key)
            .map_err(|_| AppError::bad_request("无效的token"))?;
        debug!("解密出图片token: {:?}", &image_token);

        match image_token.token_type {
            ImageTokenType::Thumbnail => {
                let bytes = state.s3_client
                    .download_with_process(&image_token.file_id, "image/resize,w_300/format,webp")
                    .await?;
                let body = Body::from(bytes);
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/webp")
                    .header(header::CACHE_CONTROL, "public, max-age=604800")
                    .body(body)
                    .unwrap())
            }
            ImageTokenType::Preview => {
                let bytes = state.s3_client
                    .download_with_process(&image_token.file_id, "image/resize,w_1920/format,webp")
                    .await?;
                let body = Body::from(bytes);
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/webp")
                    .header(header::CACHE_CONTROL, "public, max-age=604800")
                    .body(body)
                    .unwrap())
            }
            ImageTokenType::Original => {
                let bytes = state.s3_client.download(&image_token.file_id).await?;
                let content_type = get_content_type(&image_token.file_id);
                let body = Body::from(bytes);
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, content_type)
                    .header(header::CACHE_CONTROL, "public, max-age=604800")
                    .body(body)
                    .unwrap())
            }
            ImageTokenType::Crop => {
                let bbox = image_token.bbox.ok_or_else(|| AppError::bad_request("token不包含裁剪信息"))?;
                let size = 200;
                let process = format!("image/crop,x_{},y_{},w_{},h_{}/resize,w_{}/format,webp", bbox.x, bbox.y, bbox.w, bbox.h, size);
                let bytes = state.s3_client
                    .download_with_process(&image_token.file_id, &process)
                    .await?;
                let body = Body::from(bytes);
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/webp")
                    .header(header::CACHE_CONTROL, "public, max-age=604800")
                    .body(body)
                    .unwrap())
            }
        }
    }

    async fn get_thumbnail(
        State(state): State<Arc<AppState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        let image_token: ImageToken = decrypt_image_token(&token, &state.encryption_key)
            .map_err(|_| AppError::bad_request("无效的token"))?;

        let bytes = state.s3_client
            .download_with_process(&image_token.file_id, "image/resize,w_300/format,webp")
            .await?;

        let body = Body::from(bytes);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/webp")
            .header(header::CACHE_CONTROL, "public, max-age=604800")
            .body(body)
            .unwrap())
    }

    async fn get_preview(
        State(state): State<Arc<AppState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        let image_token: ImageToken = decrypt_image_token(&token, &state.encryption_key)
            .map_err(|_| AppError::bad_request("无效的token"))?;

        let bytes = state.s3_client
            .download_with_process(&image_token.file_id, "image/resize,w_1920/format,webp")
            .await?;

        let body = Body::from(bytes);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/webp")
            .header(header::CACHE_CONTROL, "public, max-age=604800")
            .body(body)
            .unwrap())
    }

    async fn get_original(
        State(state): State<Arc<AppState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        let image_token: ImageToken = decrypt_image_token(&token, &state.encryption_key)
            .map_err(|_| AppError::bad_request("无效的token"))?;

        let bytes = state.s3_client.download(&image_token.file_id).await?;

        let content_type = get_content_type(&image_token.file_id);
        let body = Body::from(bytes);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CACHE_CONTROL, "public, max-age=604800")
            .body(body)
            .unwrap())
    }

    async fn get_crop(
        State(state): State<Arc<AppState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        let image_token: ImageToken = decrypt_image_token(&token, &state.encryption_key)
            .map_err(|_| AppError::bad_request("无效的token"))?;

        let bbox = image_token.bbox.ok_or_else(|| AppError::bad_request("token不包含裁剪信息"))?;
        let size = 200;
        let process = format!("image/crop,x_{},y_{},w_{},h_{}/resize,w_{}/format,webp", bbox.x, bbox.y, bbox.w, bbox.h, size);

        let bytes = state.s3_client
            .download_with_process(&image_token.file_id, &process)
            .await?;

        let body = Body::from(bytes);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/webp")
            .header(header::CACHE_CONTROL, "public, max-age=604800")
            .body(body)
            .unwrap())
    }
}

fn get_content_type(file_id: &str) -> &'static str {
    let ext = file_id.split('.').last().unwrap_or("jpg").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    }
}
