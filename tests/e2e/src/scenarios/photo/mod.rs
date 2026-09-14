//! photo 模块 e2e 场景。
//!
//! 覆盖照片资源的读写闭环(上传 / 列表 / 详情 / 下载 / 删除)与
//! 相册、评论、点赞等子资源。共享 helper 集中在本文件:
//! - 账号:复用 `preprea` 灌入的 `loadtest_photo_*` 种子池(每账号预置 20 张照片);
//! - 内容唯一:上传内容按任务追加唯一标记,规避 `photo_photo.md5` 全局唯一约束;
//! - 数据隔离:场景自建的相册/评论/点赞由 `preprea` 在灌种子前清理。

pub mod collection;
pub mod comment;
pub mod delete;
pub mod detail;
pub mod download;
pub mod like;
pub mod list;
pub mod upload;

use std::sync::LazyLock;

use common::axum::SucR;
use memseek_test::TaskIndex;
use memseek_test::ctxlibs::http_client::HttpError;
use memseek_test::ctxlibs::http_client::multipart::{Form, Part};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types::auth::user::UserId;
use types::photo::dto::photo::PhotoView;
use types::photo::photo as photo_entity;

use crate::context::Context;

pub use crate::scenarios::user::session::{Session, login};

/// photo 测试用户池账号(对应 `preprea` 的 `loadtest_photo_{g}`)。
pub fn account(index: usize) -> String {
    format!("loadtest_photo_{}", index + 1)
}

/// 登录 photo 测试用户池账号。
pub async fn session(ctx: &Context, index: usize) -> Result<Session, HttpError> {
    login(ctx, &account(index)).await
}

/// 1x1 PNG fixture(取自 `file_validator` 单测, 可被 `FileValidator::validate_image` 解析)。
static PNG_1X1: &str = "89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415408D763F8FF7F0005FE02FE0DC444830000000049454E44AE426082";

/// 内置 PNG 字节(懒解码)。
pub static PNG_BYTES: LazyLock<Vec<u8>> =
    LazyLock::new(|| hex::decode(PNG_1X1).expect("内置 PNG fixture 非法"));

/// 生成本次运行唯一的标记:任务编号 + 轮次 + 纳秒时间戳。
///
/// 时间戳保证跨进程重启(每次 e2e 运行)也不重复, 避免 md5 唯一约束导致重复上传失败。
pub fn unique_tag(task: &TaskIndex) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}-{}", task.index, task.round, nanos)
}

/// 构造任务唯一的 PNG:在 IEND 之后追加唯一标记。
///
/// PNG 解码器读取到首个 IDAT 即停止(`image` crate 的 `into_dimensions`),
/// 追加的尾部字节不影响 IHDR 解析与尺寸校验, 但会改变内容从而改变 MD5。
pub fn unique_png(tag: &str) -> Vec<u8> {
    let mut data = PNG_BYTES.clone();
    data.extend_from_slice(b"\x00e2e:");
    data.extend_from_slice(tag.as_bytes());
    data
}

/// 构造单文件 multipart 表单。
pub fn file_form(filename: &str, content_type: &str, data: Vec<u8>) -> Result<Form, HttpError> {
    let part = Part::bytes(data)
        .file_name(filename.to_string())
        .mime_str(content_type)?;
    Ok(Form::new().part("file", part))
}

/// 计算字节内容的 md5 十六进制串(与 server 的 `md5` 计算方式一致)。
pub fn md5_hex(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

/// 上传一张图片, 返回照片视图。
pub async fn upload(
    ctx: &Context,
    session: &Session,
    filename: &str,
    data: Vec<u8>,
) -> Result<SucR<PhotoView>, HttpError> {
    let form = file_form(filename, "image/png", data)?;
    ctx.client
        .request(reqwest::Method::POST, "/photo")
        .header("Authorization", &session.auth_header())
        .multipart(form)
        .send_checked()
        .await?
        .json::<SucR<PhotoView>>()
        .await
        .map_err(HttpError::from)
}

/// 按 `seed_file_{user_ordinal}_1` 精确取一张种子照片。
///
/// 种子照片的 `file_id` 规则见 `preprea.rs`, 直接用 file_id 定位可避免依赖自增 id
/// (id 会随每次重新灌种子持续增长)。
pub async fn seed_photo(ctx: &Context, user_ordinal: u64) -> Option<photo_entity::Model> {
    photo_entity::Entity::find()
        .filter(photo_entity::Column::FileId.eq(format!("seed_file_{user_ordinal}_1")))
        .one(&ctx.db)
        .await
        .ok()
        .flatten()
}

/// 种子数据缺失时返回的错误(早于 run 失败, 提示预先灌种子)。
pub fn seed_missing() -> HttpError {
    HttpError::Status {
        status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        method: reqwest::Method::GET,
        url: "seed://missing".to_string(),
        request_body: None,
        response_body: None,
    }
}

/// 写真用:断言图片 token 与库中记录一致。
pub fn token_matches(token: Option<&String>, file_id: &str, viewer: UserId) -> bool {
    token
        .and_then(|t| types::photo::ImageToken::decrypt(t).ok())
        .is_some_and(|t| t.file_id == file_id && t.viewer_id == viewer)
}

/// 断言图片 token 可解密且绑定到指定浏览者。
pub fn token_viewer(token: Option<&String>, viewer: UserId) -> bool {
    token
        .and_then(|t| types::photo::ImageToken::decrypt(t).ok())
        .is_some_and(|t| t.viewer_id == viewer)
}

/// 当前月份的 `YYYY-MM`(以数据库 `now()` 为准, 与种子灌入保持同一时区)。
pub async fn db_current_month(ctx: &Context) -> Option<String> {
    use sea_orm::{ConnectionTrait, Statement};

    let stmt = Statement::from_string(
        ctx.db.get_database_backend(),
        "SELECT to_char(now(), 'YYYY-MM') AS m".to_owned(),
    );
    ctx.db
        .query_one_raw(stmt)
        .await
        .ok()
        .flatten()
        .and_then(|row| row.try_get::<String>("", "m").ok())
}
