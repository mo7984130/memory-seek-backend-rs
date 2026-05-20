use crate::models::photo::{
    CursorPageVO, Md5Query, PhotoCursorQuery, PhotoVO, TimeRange, UploadWithCreatedAtQuery,
};
use crate::services::photo_service::PhotoService;
use crate::state::PhotoState;
use axum::Extension;
use axum::Router;
use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::routing::{delete, get, post};
use common::error::AppError;
use common::ext::ResultErrExt;
use common::models::UserId;
use common::models::{ImageToken, ImageTokenType};
use common::r::R;
use std::sync::Arc;
use std::vec;
use tracing::debug;

pub struct PhotoController;

#[derive(Clone, Copy)]
enum ImageDownloadType {
    Thumbnail,
    Preview,
    Original,
    Crop,
}

impl PhotoController {
    /// 构建需要认证的照片相关路由
    ///
    /// # 返回
    /// 包含上传、查询、删除等路由的 `Router`
    pub fn routes() -> Router<Arc<PhotoState>> {
        Router::new()
            .route("/upload", post(Self::upload))
            .route(
                "/upload/with-created-at",
                post(Self::upload_with_created_at),
            )
            .route("/cursor", get(Self::get_photos_cursor))
            .route("/md5-exist", get(Self::md5_exist))
            .route("/time-range", get(Self::get_time_range))
            .route("/{id}", delete(Self::delete_photo))
    }

    /// 构建公开的图片访问路由（无需认证）
    ///
    /// # 返回
    /// 包含图片获取、缩略图、预览图、原图、裁剪图等路由的 `Router`
    pub fn public_routes() -> Router<Arc<PhotoState>> {
        Router::new()
            .route("/{token}", get(Self::get_image))
            .route("/{token}/thumbnail", get(Self::get_thumbnail))
            .route("/{token}/preview", get(Self::get_preview))
            .route("/{token}/original", get(Self::get_original))
            .route("/{token}/crop", get(Self::get_crop))
    }

    /// 上传照片
    ///
    /// # 参数
    /// - `state`: 应用状态，包含数据库连接和存储客户端
    /// - `user_id`: 当前认证用户的 ID
    /// - `multipart`: multipart 表单数据，包含上传的文件
    ///
    /// # 返回
    /// 返回上传成功后的照片信息
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 表单数据无效或未找到上传文件
    /// - `AppError`: 文件读取或上传失败
    async fn upload(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        mut multipart: Multipart,
    ) -> Result<R<PhotoVO>, AppError> {
        let field = multipart
            .next_field()
            .await
            .map_err(|_| AppError::bad_request("无效的表单数据"))?
            .ok_or_else(|| AppError::bad_request("未找到上传文件"))?;

        let file_name = field
            .file_name()
            .unwrap_or("photo_entities.jpg")
            .to_string();
        let content_type = field.content_type().unwrap_or("image/jpg").to_string();
        let file_data = field
            .bytes()
            .await
            .trace_to_internal_err("read_file_err", "读取文件失败")?;

        let photo =
            PhotoService::upload_photo(&state, user_id.0, file_data, file_name, content_type, None)
                .await?;

        Ok(R::ok(photo))
    }

    /// 上传照片并指定创建时间（仅管理员可用）
    ///
    /// # 参数
    /// - `state`: 应用状态，包含数据库连接和存储客户端
    /// - `user_id`: 当前认证用户的 ID（必须为管理员）
    /// - `query`: 查询参数，包含自定义的创建时间
    /// - `multipart`: multipart 表单数据，包含上传的文件
    ///
    /// # 返回
    /// 返回上传成功后的照片信息
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 非管理员用户、表单数据无效或未找到上传文件
    /// - `AppError`: 文件读取或上传失败
    async fn upload_with_created_at(
        State(state): State<Arc<PhotoState>>,
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

        let file_name = field
            .file_name()
            .unwrap_or("photo_entities.jpg")
            .to_string();
        let content_type = field.content_type().unwrap_or("image/jpeg").to_string();
        let file_data = field
            .bytes()
            .await
            .trace_to_internal_err("read_file_err", "读取文件失败")?;

        let photo = PhotoService::upload_photo(
            &state,
            user_id.0,
            file_data,
            file_name,
            content_type,
            Some(query.created_at),
        )
        .await?;

        Ok(R::ok(photo))
    }

    /// 使用游标分页获取当前用户的照片列表
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `query`: 游标分页查询参数
    ///
    /// # 返回
    /// 返回游标分页的照片列表
    ///
    /// # 错误
    /// - `AppError`: 查询失败
    async fn get_photos_cursor(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Query(query): Query<PhotoCursorQuery>,
    ) -> Result<R<CursorPageVO<PhotoVO, String>>, AppError> {
        let result = PhotoService::get_photo_cursor_page(&state, user_id.0, query).await?;
        Ok(R::ok(result))
    }

    /// 批量检查 MD5 对应的照片是否已存在
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `params`: 包含多个 MD5 值的查询参数
    /// - `user_id`: 当前认证用户的 ID
    ///
    /// # 返回
    /// 返回与输入 MD5 列表对应的布尔值列表，表示每个 MD5 是否已存在
    ///
    /// # 错误
    /// - `AppError`: 查询失败
    async fn md5_exist(
        State(state): State<Arc<PhotoState>>,
        Query(params): Query<Md5Query>,
        Extension(user_id): Extension<UserId>,
    ) -> Result<R<Vec<bool>>, AppError> {
        let exists = PhotoService::exists_by_md5_batch(&state, user_id, &params.md5).await?;
        Ok(R::ok(exists))
    }

    /// 获取所有照片的时间范围（最早和最晚的拍摄时间）
    ///
    /// # 参数
    /// - `state`: 应用状态
    ///
    /// # 返回
    /// 返回照片的最早和最晚时间范围
    ///
    /// # 错误
    /// - `AppError`: 查询失败
    async fn get_time_range(
        State(state): State<Arc<PhotoState>>,
    ) -> Result<R<TimeRange>, AppError> {
        let time_range = PhotoService::get_time_range(&state).await?;
        Ok(R::ok(time_range))
    }

    /// 删除指定照片
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `user_id`: 当前认证用户的 ID
    /// - `id`: 要删除的照片 ID（路径参数）
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 照片 ID 格式无效
    /// - `AppError`: 删除失败或无权删除
    async fn delete_photo(
        State(state): State<Arc<PhotoState>>,
        Extension(user_id): Extension<UserId>,
        Path(id): Path<String>,
    ) -> Result<R<()>, AppError> {
        let photo_id: i64 = id
            .parse()
            .map_err(|_| AppError::bad_request("无效的照片ID"))?;

        PhotoService::delete_photos(&state, user_id, vec![photo_id]).await?;

        Ok(R::ok(()))
    }

    // 处理图片下载请求，根据下载类型从 OSS 获取对应规格的图片
    //
    // # 参数
    // - `state`: 应用状态
    // - `token`: 加密的图片 token
    // - `download_type`: 下载类型（缩略图/预览图/原图/裁剪图）
    //
    // # 返回
    // 返回包含图片数据和正确 Content-Type 的 HTTP 响应
    //
    // # 错误
    // - `AppError::BadRequest`: token 无效或不包含裁剪信息
    // - `AppError`: OSS 下载失败
    async fn handle_image_download(
        state: &Arc<PhotoState>,
        token: &str,
        download_type: ImageDownloadType,
    ) -> Result<Response<Body>, AppError> {
        let image_token: ImageToken = state
            .token_cipher
            .decrypt(token)
            .map_err(|_| AppError::bad_request("无效的token"))?;

        let (bytes, content_type) = match download_type {
            ImageDownloadType::Thumbnail => {
                let bytes = state
                    .s3_client
                    .download_with_process(&image_token.file_id, "image/resize,w_300/format,webp")
                    .await?;
                (bytes, "image/webp")
            }
            ImageDownloadType::Preview => {
                let bytes = state
                    .s3_client
                    .download_with_process(&image_token.file_id, "image/resize,w_1920/format,webp")
                    .await?;
                (bytes, "image/webp")
            }
            ImageDownloadType::Original => {
                let bytes = state.s3_client.download(&image_token.file_id).await?;
                let content_type = get_content_type(&image_token.file_id);
                (bytes, content_type)
            }
            ImageDownloadType::Crop => {
                let bbox = image_token
                    .bbox
                    .ok_or_else(|| AppError::bad_request("token不包含裁剪信息"))?;
                let size = 200;
                let process = format!(
                    "image/crop,x_{},y_{},w_{},h_{}/resize,w_{}/format,webp",
                    bbox.x, bbox.y, bbox.w, bbox.h, size
                );
                let bytes = state
                    .s3_client
                    .download_with_process(&image_token.file_id, &process)
                    .await?;
                (bytes, "image/webp")
            }
        };

        let body = Body::from(bytes);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CACHE_CONTROL, "public, max-age=604800")
            .body(body)
            .unwrap())
    }

    /// 根据 token 类型自动获取对应规格的图片
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `token`: 加密的图片 token，内含类型信息
    ///
    /// # 返回
    /// 返回对应规格的图片 HTTP 响应
    ///
    /// # 错误
    /// - `AppError::BadRequest`: token 无效
    /// - `AppError`: 图片获取失败
    async fn get_image(
        State(state): State<Arc<PhotoState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        let image_token: ImageToken = state
            .token_cipher
            .decrypt(&token)
            .map_err(|_| AppError::bad_request("无效的token"))?;
        debug!("解密出图片token: {:?}", &image_token);

        let download_type = match image_token.token_type {
            ImageTokenType::Thumbnail => ImageDownloadType::Thumbnail,
            ImageTokenType::Preview => ImageDownloadType::Preview,
            ImageTokenType::Original => ImageDownloadType::Original,
            ImageTokenType::Crop => ImageDownloadType::Crop,
        };

        Self::handle_image_download(&state, &token, download_type).await
    }

    /// 获取缩略图（300px 宽，webp 格式）
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `token`: 加密的图片 token
    ///
    /// # 返回
    /// 返回缩略图的 HTTP 响应
    ///
    /// # 错误
    /// - `AppError::BadRequest`: token 无效
    /// - `AppError`: 图片获取失败
    async fn get_thumbnail(
        State(state): State<Arc<PhotoState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        Self::handle_image_download(&state, &token, ImageDownloadType::Thumbnail).await
    }

    /// 获取预览图（1920px 宽，webp 格式）
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `token`: 加密的图片 token
    ///
    /// # 返回
    /// 返回预览图的 HTTP 响应
    ///
    /// # 错误
    /// - `AppError::BadRequest`: token 无效
    /// - `AppError`: 图片获取失败
    async fn get_preview(
        State(state): State<Arc<PhotoState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        Self::handle_image_download(&state, &token, ImageDownloadType::Preview).await
    }

    /// 获取原始图片（未经处理）
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `token`: 加密的图片 token
    ///
    /// # 返回
    /// 返回原始图片的 HTTP 响应
    ///
    /// # 错误
    /// - `AppError::BadRequest`: token 无效
    /// - `AppError`: 图片获取失败
    async fn get_original(
        State(state): State<Arc<PhotoState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        Self::handle_image_download(&state, &token, ImageDownloadType::Original).await
    }

    /// 获取裁剪后的图片（200px 宽，webp 格式）
    ///
    /// # 参数
    /// - `state`: 应用状态
    /// - `token`: 加密的图片 token，需包含裁剪区域信息
    ///
    /// # 返回
    /// 返回裁剪后图片的 HTTP 响应
    ///
    /// # 错误
    /// - `AppError::BadRequest`: token 无效或不包含裁剪信息
    /// - `AppError`: 图片获取失败
    async fn get_crop(
        State(state): State<Arc<PhotoState>>,
        Path(token): Path<String>,
    ) -> Result<Response<Body>, AppError> {
        Self::handle_image_download(&state, &token, ImageDownloadType::Crop).await
    }
}

// 根据文件扩展名返回对应的 Content-Type
//
// # 参数
// - `file_id`: 文件 ID 或路径，用于提取扩展名
//
// # 返回
// 返回对应的 MIME 类型字符串，未知扩展名默认返回 "image/jpeg"
fn get_content_type(file_id: &str) -> &'static str {
    let ext = file_id.split('.').next_back().unwrap_or("jpg").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    }
}
