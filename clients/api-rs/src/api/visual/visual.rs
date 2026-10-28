//! 影像相关 API(图片 / 视频)。
//!
//! 路由前缀:`/visual`。

use crate::client::{Client, Request};
use crate::error::Result;
use crate::transport::{DownloadResponse, HttpResponse, Method, StatusCode};
use common_core::time::DateTime;
use common_core::types::CursorPage;
use types_core::cursor::TimeIdCursor;
use types_core::{PageDirection, VisualId};
use types_visual::dto::visual::VisualView;
use types_visual::models::{DeleteVisualsParam, ExistsByHashBatchParam, VisualIds};

/// 上传影像(图片 / 视频)。`POST /visual`
///
/// 请求体即文件字节(非 multipart);文件名与 MIME 由后端按文件头魔数嗅探,
/// `created_at` 仅管理员可设置。
pub async fn upload_visual(
    client: &Client,
    bytes: Vec<u8>,
    content_type: Option<String>,
    created_at: Option<DateTime>,
) -> Result<VisualView> {
    let mut request = Request::post("/visual").bytes(bytes, content_type);
    if let Some(created_at) = created_at {
        request = request.query("createdAt", created_at);
    }
    client.send_json(request).await
}

/// 获取影像列表(游标分页)。`GET /visual`
pub async fn get_visuals(
    client: &Client,
    cursor: Option<TimeIdCursor<VisualId>>,
    size: Option<u64>,
    direction: Option<PageDirection>,
    anchor_time: Option<DateTime>,
) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
    let mut request = Request::get("/visual");
    if let Some(cursor) = cursor {
        request = request.query("cursor", cursor);
    }
    if let Some(size) = size {
        request = request.query("size", size);
    }
    if let Some(direction) = direction {
        request = request.query_raw("direction", direction_str(direction));
    }
    if let Some(anchor_time) = anchor_time {
        request = request.query("anchorTime", anchor_time);
    }
    client.send_json(request).await
}

/// 获取单张影像信息。`GET /visual/visual/{visualId}`
pub async fn get_visual_info(client: &Client, visual_id: VisualId) -> Result<VisualView> {
    client
        .send_json(Request::get(format!("/visual/visual/{visual_id}")))
        .await
}

/// 批量校验内容哈希(blake3)是否存在。`POST /visual/check-existence`
pub async fn check_hashes_exist(client: &Client, hashes: Vec<String>) -> Result<Vec<bool>> {
    let param = ExistsByHashBatchParam { hashes };
    client
        .send_json(Request::post("/visual/check-existence").json(&param))
        .await
}

/// 批量删除影像。`DELETE /visual`
pub async fn delete_visuals(client: &Client, visual_ids: VisualIds) -> Result<()> {
    let param = DeleteVisualsParam { visual_ids };
    client
        .send_unit(Request::delete("/visual").json(&param))
        .await
}

/// 凭 token 生成影像访问 URL(图片 / 视频封面 / 原视频流)。
pub fn visual_url(client: &Client, token: &str) -> String {
    client.visual_url(token)
}

/// 下载影像资源(原图 / 缩略图 / 裁剪 / 视频衍生片)。`GET /visual/{token}`(公开)。
///
/// 返回原始 [`HttpResponse`],供调用方读取状态码与响应头(如 `Content-Type`)
/// 并处理二进制内容;该端点直接返回文件字节,不经 `R` 包壳解析。
pub async fn download(client: &Client, token: &str) -> Result<HttpResponse> {
    client
        .raw(Method::GET, format!("/visual/{token}"))
        .no_auth()
        .send()
        .await
}

/// 把影像资源流式写入 `dest`(原图 / 缩略图 / 裁剪 / 视频衍生片)。
///
/// 与 [`download`] 相同端点,但支持流式的 [`Transport`](crate::Transport) 实现会边读
/// 响应体边写文件,不会把整个原视频放进内存。非 2xx 不会创建/保留目标文件;
/// 视频衍生片返回 `202 Accepted`(转码未就绪)时也会清理空文件,可通过
/// [`DownloadResponse::is_ready`] 判断内容是否可用。
pub async fn download_to(
    client: &Client,
    token: &str,
    dest: impl AsRef<std::path::Path>,
) -> Result<DownloadResponse> {
    let dest = dest.as_ref();
    let response = client
        .raw(Method::GET, format!("/visual/{token}"))
        .no_auth()
        .download_to(dest)
        .await?;
    if response.status == StatusCode::ACCEPTED {
        let _ = std::fs::remove_file(dest);
    }
    Ok(response)
}

/// 以单段 HTTP Range 下载影像资源(如 `bytes=0-499`)。`GET /visual/{token}`
///
/// 返回原始 [`HttpResponse`];后端支持 `206 Partial Content` 与 `Content-Range`。
pub async fn download_range(client: &Client, token: &str, range: &str) -> Result<HttpResponse> {
    client
        .raw(Method::GET, format!("/visual/{token}"))
        .no_auth()
        .header("Range", range)
        .send()
        .await
}

/// 以单段 HTTP Range 流式写入 `dest`。`GET /visual/{token}`
///
/// 与 [`download_to`] 相同,但额外携带 `Range` 头,适用于按分片读取大视频。
pub async fn download_range_to(
    client: &Client,
    token: &str,
    range: &str,
    dest: impl AsRef<std::path::Path>,
) -> Result<DownloadResponse> {
    let dest = dest.as_ref();
    let response = client
        .raw(Method::GET, format!("/visual/{token}"))
        .no_auth()
        .header("Range", range)
        .download_to(dest)
        .await?;
    if response.status == StatusCode::ACCEPTED {
        let _ = std::fs::remove_file(dest);
    }
    Ok(response)
}

/// 分页方向 → 线协议字符串(后端 `#[serde(rename_all = "lowercase")]`)。
fn direction_str(direction: PageDirection) -> &'static str {
    match direction {
        PageDirection::Next => "next",
        PageDirection::Prev => "prev",
    }
}
