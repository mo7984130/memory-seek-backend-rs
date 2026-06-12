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
    models::{CursorPage, ImageToken, ImageTokenType},
    traits::controller::ControllerRouter,
};
use common::{ext::OptionExt, r::R};
use entities::auth::user::UserId;
use futures::StreamExt;

use crate::{
    models::photo::{DeletePhotoParam, Md5sExistParam, PhotoCursorParam, PhotoResult},
    services::photo_service::PhotoService,
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
    ) -> Result<R<PhotoResult>> {
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

        PhotoService::upload_photo(&state, user_id, file_data, file_name, content_type, None)
            .await
            .to_r_ok()
    }

    async fn get_photos_cursor(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(query): ValidatedQuery<PhotoCursorParam>,
    ) -> Result<R<CursorPage<PhotoResult, String>>> {
        PhotoService::get_photo_cursor_page(&state, user_id, query)
            .await
            .to_r_ok()
    }

    async fn md5s_exist(
        State(state): State<Arc<PhotoState>>,
        ValidatedJson(data): ValidatedJson<Md5sExistParam>,
    ) -> Result<R<Vec<bool>>> {
        PhotoService::exists_by_md5_batch(&state, &data.md5s)
            .await
            .to_r_ok()
    }

    async fn get_image(
        State(state): State<Arc<PhotoState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>> {
        let image_token: ImageToken = state.token_cipher.decrypt(&token).trace_warn_bad_request(
            "invalid_image_token",
            "无效的token",
            "无效的token",
        )?;

        Self::handle_image_download(state, image_token).await
    }

    async fn handle_image_download(
        state: Arc<PhotoState>,
        token: ImageToken,
    ) -> Result<Response<Body>> {
        match token.token_type {
            ImageTokenType::Thumbnail | ImageTokenType::Preview | ImageTokenType::Crop => {
                let process_param: String = match token.token_type {
                    ImageTokenType::Thumbnail => "image/resize,w_300/format,webp".to_string(),
                    ImageTokenType::Preview => "image/resize,w_1920/format,webp".to_string(),
                    ImageTokenType::Crop => {
                        let bbox = token.bbox.ok_or_warn_bad_request(
                            "image_token_crop_info_not_found",
                            "token里面没有包含裁剪信息",
                            "token不包含裁剪信息",
                        )?;
                        let size = 200;
                        format!(
                            "image/crop,x_{},y_{},w_{},h_{}/resize,w_{}/format,webp",
                            bbox.x, bbox.y, bbox.w, bbox.h, size
                        )
                    }
                    _ => unreachable!(),
                };
                let bytes = state
                    .s3_client
                    .download_with_process(&token.file_id, &process_param)
                    .await?;

                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/webp")
                    .header(header::CACHE_CONTROL, "public, max-age=604800")
                    .body(Body::from(bytes))
                    .unwrap())
            }
            ImageTokenType::Original => {
                let stream_resp = state
                    .s3_client
                    .get_download_stream_response(&token.file_id)
                    .await?;

                let stream = stream_resp
                    .bytes
                    .map(|chunk| chunk.trace_internal_err("oss_stream_err", "OSS流读取失败"));

                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, Self::get_content_type(&token.file_id))
                    .header(header::CACHE_CONTROL, "public, max-age=604800")
                    .body(Body::from_stream(stream))
                    .unwrap())
            }
        }
    }

    fn get_content_type(file_id: &str) -> &'static str {
        let ext = file_id
            .split('.')
            .next_back()
            .unwrap_or("jpg")
            .to_lowercase();
        match ext.as_str() {
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            _ => "image/jpeg",
        }
    }

    async fn delete_photos(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedJson(data): ValidatedJson<DeletePhotoParam>,
    ) -> Result<R<()>> {
        PhotoService::delete_photos(&state, user_id, data.photo_ids)
            .await
            .to_r_ok()
    }
}
