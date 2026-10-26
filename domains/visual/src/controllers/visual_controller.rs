use std::sync::Arc;

use axum::{
    Extension, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use common_core::{Result, types::CursorPage};
use common_web::{
    R, controller_router::ControllerRouter, ext::ToROkExt, extractors::ValidatedJson,
    extractors::ValidatedPath, extractors::ValidatedQuery,
};
use types_core::cursor::TimeIdCursor;
use types_identity::auth::user::UserId;
use types_visual::{
    VisualToken,
    dto::visual::{VisualCursorParam, VisualView},
    models::{DeleteVisualsParam, ExistsByHashBatchParam, UploadVisualParam},
    visual::VisualId,
};

use crate::{
    services::visual_service::{ImageDownloadData, VisualService},
    state::VisualState,
};

pub struct VisualController;

impl ControllerRouter for VisualController {
    type State = VisualState;

    /// 影像模块的受保护路由
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

    /// 影像模块的公开路由，凭 token 访问单张影像
    fn public_routes() -> Router<Arc<VisualState>> {
        Router::new().route("/{token}", get(Self::get_visual))
    }
}

impl VisualController {
    /// 接收请求体的原始字节流作为影像，完成校验、存储并记录上传行为；`created_at` 仅管理员可经 query 传入
    async fn upload(
        State(state): State<Arc<VisualState>>,
        Extension(user_id): Extension<UserId>,
        ValidatedQuery(req): ValidatedQuery<UploadVisualParam>,
        body: Body,
    ) -> Result<R<VisualView>> {
        let visual =
            VisualService::upload_from_body(Arc::clone(&state), user_id, req, body).await?;
        Ok(visual).to_r_ok()
    }

    /// 游标获取影像列表
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

    /// 批量检查影像哈希值是否已存在
    async fn hashes_exist(
        State(state): State<Arc<VisualState>>,
        ValidatedJson(req): ValidatedJson<ExistsByHashBatchParam>,
    ) -> Result<R<Vec<bool>>> {
        VisualService::exists_by_hash_batch(&state, req)
            .await
            .to_r_ok()
    }

    /// 解密视觉访问令牌并返回原图/原视频或处理后的影像流(图片处理/视频截帧)
    async fn get_visual(
        State(state): State<Arc<VisualState>>,
        Path(token): Path<String>,
        headers: HeaderMap,
    ) -> Result<Response<Body>> {
        let visual_token: VisualToken = VisualToken::decrypt(&token)?;
        // Range 只对视频有意义; 图片一律返回完整内容
        let supports_range = visual_token.kind.is_video();
        let range = if supports_range {
            single_byte_range(&headers)
        } else {
            None
        };

        let data = VisualService::download_visual(&state, visual_token, range).await?;

        let resp = match data {
            ImageDownloadData::Buffered {
                bytes,
                content_type,
            } => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CACHE_CONTROL, "public, max-age=604800")
                .body(Body::from(bytes))
                .unwrap(),
            ImageDownloadData::Streamed {
                stream,
                content_type,
                status,
                content_range,
                content_length,
            } => {
                // 图片不支持 Range, 统一按完整内容返回
                let status = if supports_range {
                    StatusCode::from_u16(status).unwrap_or(StatusCode::OK)
                } else {
                    StatusCode::OK
                };

                // 范围越界: 返回 416 与对象总长度, 响应体为空
                if supports_range && status == StatusCode::RANGE_NOT_SATISFIABLE {
                    let mut builder = Response::builder()
                        .status(status)
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(header::CACHE_CONTROL, "public, max-age=604800");
                    if let Some(content_range) = content_range {
                        builder = builder.header(header::CONTENT_RANGE, content_range);
                    }
                    return Ok(builder.body(Body::empty()).unwrap());
                }

                let mut builder = Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, content_type)
                    .header(header::CACHE_CONTROL, "public, max-age=604800");
                if supports_range {
                    builder = builder.header(header::ACCEPT_RANGES, "bytes");
                    if let Some(content_range) = content_range {
                        builder = builder.header(header::CONTENT_RANGE, content_range);
                    }
                }
                if let Some(content_length) = content_length {
                    builder = builder.header(header::CONTENT_LENGTH, content_length);
                }
                builder.body(Body::from_stream(stream)).unwrap()
            }
            // 视频衍生片仍在异步转码, 告知客户端稍后重试
            ImageDownloadData::NotReady => Response::builder()
                .status(StatusCode::ACCEPTED)
                .header(header::RETRY_AFTER, "3")
                .body(Body::empty())
                .unwrap(),
        };

        Ok(resp)
    }

    /// 删除当前用户指定的影像及其对象存储文件
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

/// 提取单段 `Range` 头(如 `bytes=0-499`); 多段或格式非法时忽略, 按整对象返回
fn single_byte_range(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::RANGE)?.to_str().ok()?.trim();
    let spec = value.strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }

    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    let (first, last) = spec.split_once('-')?;
    let valid = match (first, last) {
        ("", suffix) => digits(suffix) && suffix.parse::<u64>().ok()? > 0,
        (start, "") => digits(start) && start.parse::<u64>().is_ok(),
        (start, end) => {
            digits(start) && digits(end) && start.parse::<u64>().ok()? <= end.parse::<u64>().ok()?
        }
    };

    valid.then_some(value)
}

#[cfg(test)]
mod tests {
    use axum::http::header;

    use super::*;

    fn range_header(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, value.parse().unwrap());
        headers
    }

    #[test]
    fn accepts_single_byte_range() {
        assert_eq!(
            single_byte_range(&range_header("bytes=0-499")),
            Some("bytes=0-499")
        );
        assert_eq!(
            single_byte_range(&range_header("bytes=500-")),
            Some("bytes=500-")
        );
        assert_eq!(
            single_byte_range(&range_header("bytes=-500")),
            Some("bytes=-500")
        );
    }

    #[test]
    fn ignores_multi_range() {
        assert_eq!(single_byte_range(&range_header("bytes=0-1,3-4")), None);
    }

    #[test]
    fn ignores_invalid_range() {
        assert_eq!(single_byte_range(&range_header("items=0-1")), None);
        assert_eq!(single_byte_range(&range_header("bytes=abc")), None);
        assert_eq!(single_byte_range(&range_header("bytes=5-1")), None);
        assert_eq!(single_byte_range(&range_header("bytes=-")), None);
        assert_eq!(single_byte_range(&range_header("bytes=-0")), None);
    }

    #[test]
    fn ignores_absent_range() {
        assert_eq!(single_byte_range(&HeaderMap::new()), None);
    }
}
