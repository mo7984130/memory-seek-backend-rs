//! visual 模块 e2e 场景。
//!
//! 覆盖影像资源的读写闭环(上传 / 列表 / 详情 / 下载 / 删除)与
//! 相册、评论、点赞等子资源。共享 helper 集中在本文件:
//! - 账号:复用 `preprea` 灌入的 `loadtest_visual_*` 种子池(每账号预置 20 张影像);
//! - 内容唯一:上传内容按任务追加唯一标记,规避 `visual_visual.hash` 全局唯一约束;
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
use memseek_test::ctxlibs::http_client::{HttpError, reqwest};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types::auth::user::UserId;
use types::visual::dto::visual::VisualView;
use types::visual::visual as visual_entity;

use crate::context::Context;

pub use crate::scenarios::user::session::{Session, login};

/// visual 测试用户池账号(对应 `preprea` 的 `loadtest_visual_{g}`)。
pub fn account(index: usize) -> String {
    format!("loadtest_visual_{}", index + 1)
}

/// 登录 visual 测试用户池账号。
pub async fn session(ctx: &Context, index: usize) -> Result<Session, HttpError> {
    login(ctx, &account(index)).await
}

/// 1x1 PNG fixture(结构合法、可完整解码; 三个 chunk 的 CRC 均正确)。
///
/// 必须是能真正解码的影像: `FileValidator` 只读文件头/尺寸(IHDR)即放行,
/// 但 face 管线会用 `image::load_from_memory` 做完整解码。
static PNG_1X1: &str = "89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415478DA63F8CFC0000003010100F70341430000000049454E44AE426082";

/// 内置 PNG 字节(懒解码)。
pub static PNG_BYTES: LazyLock<Vec<u8>> =
    LazyLock::new(|| hex::decode(PNG_1X1).expect("内置 PNG fixture 非法"));

/// 生成本次运行唯一的标记:任务编号 + 轮次 + 纳秒时间戳。
///
/// 时间戳保证跨进程重启(每次 e2e 运行)也不重复, 避免 hash 唯一约束导致重复上传失败。
pub fn unique_tag(task: &TaskIndex) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}-{}", task.index, task.round, nanos)
}

/// 构造任务唯一的 PNG:在 IEND 之后追加唯一标记。
///
/// 解码器读取到 IEND 即停止(`image` crate 的完整解码同样如此), 追加的尾部字节
/// 不影响解码与尺寸校验, 但会改变内容从而改变哈希。
pub fn unique_png(tag: &str) -> Vec<u8> {
    let mut data = PNG_BYTES.clone();
    data.extend_from_slice(b"\x00e2e:");
    data.extend_from_slice(tag.as_bytes());
    data
}

/// 上传一个影像(request body 即文件字节), 返回影像视图。
pub async fn upload(
    ctx: &Context,
    session: &Session,
    data: Vec<u8>,
) -> Result<SucR<VisualView>, HttpError> {
    ctx.client
        .request(reqwest::Method::POST, "/visual")
        .header("Authorization", &session.auth_header())
        .header("content-type", "image/png")
        .body(data)
        .send_checked()
        .await?
        .json::<SucR<VisualView>>()
        .await
        .map_err(HttpError::from)
}

/// 计算字节内容的 blake3 十六进制串(与 server 的 `blake3` 计算方式一致)。
pub fn blake3_hex(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// 按 `seed_file_{user_ordinal}_1` 精确取一张种子影像。
///
/// 种子影像的 `file_id` 规则见 `preprea.rs`, 直接用 file_id 定位可避免依赖自增 id
/// (id 会随每次重新灌种子持续增长)。
pub async fn seed_visual(ctx: &Context, user_ordinal: u64) -> Option<visual_entity::Model> {
    visual_entity::Entity::find()
        .filter(visual_entity::Column::FileId.eq(format!("seed_file_{user_ordinal}_1")))
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

/// 写真用:断言视觉 token 与库中记录一致。
pub fn token_matches(token: Option<&String>, file_id: &str, viewer: UserId) -> bool {
    token
        .and_then(|t| types::visual::VisualToken::decrypt(t).ok())
        .is_some_and(|t| t.file_id == file_id && t.viewer_id == viewer)
}

/// 断言视觉 token 可解密且绑定到指定浏览者。
pub fn token_viewer(token: Option<&String>, viewer: UserId) -> bool {
    token
        .and_then(|t| types::visual::VisualToken::decrypt(t).ok())
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
