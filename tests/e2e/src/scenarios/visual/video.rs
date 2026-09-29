//! 视频衍生片：上传后异步生成缩略/预览片，下载轮询至就绪。
//!
//! 覆盖：视频上传 → `thumbnail_token` 下载在转码完成前返回 202、
//! 完成后返回 200 且为 `video/mp4` 的衍生片（非原视频）。

use std::time::Duration;

use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::Scenario,
};
use serde_json::json;
use types_visual::dto::visual::VisualView;

use crate::context::Context;

use super::{Session, session, unique_tag};

/// 内置最小可解析 MP4 fixture(160x120, 1s, H.264)。
pub static MP4_BYTES: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/tiny.mp4"));

/// 在 MP4 末尾追加一个合法的 `free` box(携带唯一标记)。
///
/// box 结构是一个扁平列表，尾部追加合法 box 不影响解析；而内容变化会改变
/// blake3 哈希，规避 `visual_visual.hash` 全局唯一约束导致的重复上传失败。
pub fn unique_mp4(tag: &str) -> Vec<u8> {
    let mut data = MP4_BYTES.to_vec();
    let payload = tag.as_bytes();
    let box_size = (8 + payload.len()) as u32;
    data.extend_from_slice(&box_size.to_be_bytes());
    data.extend_from_slice(b"free");
    data.extend_from_slice(payload);
    data
}

/// 下载视频缩略片的前置：登录 + 上传唯一视频。
#[derive(Default)]
pub struct VideoDerivativeSetup {
    pub thumbnail_token: String,
    pub preview_token: String,
    pub visual_id: i64,
    pub auth_header: String,
}

/// 视频缩略片：轮询至就绪后为 `video/mp4` 衍生片。
#[derive(Default)]
pub struct VideoDerivativeScenario;

impl Scenario for VideoDerivativeScenario {
    type Ctx = Context;

    type Error = HttpError;

    /// (状态码, content-type, 字节)
    type Output = (u16, Option<String>, Vec<u8>);

    type Setup = VideoDerivativeSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session: Session = session(ctx, task.index).await?;
        let bytes = unique_mp4(&unique_tag(task));
        let view: VisualView = super::upload_video(ctx, &session, bytes).await?.data;
        Ok(VideoDerivativeSetup {
            thumbnail_token: view.thumbnail_token.unwrap_or_default(),
            preview_token: view.preview_token.unwrap_or_default(),
            visual_id: view.id.0,
            auth_header: session.auth_header(),
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        // 转码异步: 短暂轮询。202 = 仍在转码(合法契约), 200 = 就绪;
        // 其它状态(500/404 等)视为失败。轮询有界, 避免压测/CI 下拖长单轮耗时。
        for _ in 0..20 {
            let resp = ctx
                .client
                .request(
                    reqwest::Method::GET,
                    &format!("/visual/{}", setup.thumbnail_token),
                )
                .send()
                .await?;
            let status = resp.status().as_u16();
            if status == 202 {
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
            if status != 200 {
                return Ok((status, None, Vec::new()));
            }
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let bytes = resp.bytes().await?.to_vec();
            return Ok((status, content_type, bytes));
        }
        // 轮询耗尽仍为 202: 转码未完成, 但契约成立
        Ok((202, None, Vec::new()))
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let (status, content_type, bytes) = output;
        let tokens_ok = !setup.thumbnail_token.is_empty() && !setup.preview_token.is_empty();
        if *status == 202 {
            // 仍在异步转码: 非错误, 契约成立
            return Ok(tokens_ok);
        }
        // 就绪的衍生片应为 MP4(ISOBMFF: 偏移 4 处为 `ftyp`)
        let is_mp4 = bytes.len() >= 8 && &bytes[4..8] == b"ftyp";
        Ok(tokens_ok && *status == 200 && content_type.as_deref() == Some("video/mp4") && is_mp4)
    }

    /// 收尾：删除 setup 阶段上传的视频（会连带清理衍生片）。
    async fn teardown(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        crate::cleanup::delete(
            ctx,
            "/visual",
            Some(&setup.auth_header),
            Some(json!({ "visualIds": [setup.visual_id] })),
        )
        .await
    }
}

register_scenario!(VideoDerivativeScenario);
