use std::sync::LazyLock;

use common::axum::{ErrR, SucR};
use memseek_test::ctxlibs::http_client::multipart::{Form, Part};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::{HttpError, reqwest},
    register_scenario,
    scenario::Scenario,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types::auth;
use types::visual::VisualTokenStr;

use crate::context::Context;

use super::session::{Session, login, user_account};

/// 1x1 PNG fixture(结构合法、可完整解码; 三个 chunk 的 CRC 均正确).
static PNG_1X1: &str = "89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415478DA63F8CFC0000003010100F70341430000000049454E44AE426082";

/// 内置 PNG 字节(懒解码).
static PNG_BYTES: LazyLock<Vec<u8>> =
    LazyLock::new(|| hex::decode(PNG_1X1).expect("内置 PNG fixture 非法"));

/// 构造单文件 multipart 表单(用库重新导出的 `reqwest::multipart` 类型).
fn file_form(filename: &str, content_type: &str, data: Vec<u8>) -> Result<Form, HttpError> {
    let part = Part::bytes(data)
        .file_name(filename.to_string())
        .mime_str(content_type)?;
    Ok(Form::new().part("file", part))
}

/// 上传头像: 落库 key 与响应 token 一致, 且 S3 对象存在、内容与上传一致.
#[derive(Default)]
pub struct UploadAvatarScenario;

impl Scenario for UploadAvatarScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<VisualTokenStr>;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &user_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let form = file_form("avatar.png", "image/png", PNG_BYTES.clone())?;
        ctx.client
            .request(reqwest::Method::PUT, "/user/avatar")
            .header("Authorization", &setup.auth_header())
            .multipart(form)
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let token = &output.data.0;
        let prefix_ok = token
            .file_id
            .starts_with(&format!("avatars/{}/", setup.user_id.0))
            && token.file_id.ends_with(".png");
        let viewer_ok = token.viewer_id == setup.user_id;

        let db_file = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.user_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .and_then(|u| u.avatar_file_id);
        let db_ok = db_file.as_deref() == Some(token.file_id.as_str());

        // S3 回查: 对象已落盘且内容与上传字节一致
        let s3_ok = ctx.s3_bytes(&token.file_id).await.as_deref() == Some(PNG_BYTES.as_slice());

        Ok(prefix_ok && viewer_ok && db_ok && s3_ok)
    }
}

register_scenario!(UploadAvatarScenario);

#[derive(Default)]
pub struct ReplaceSetup {
    pub session: Session,
    pub old_key: String,
}

/// 重复上传头像: 新对象存在、旧对象被删除、库中指向新 key.
#[derive(Default)]
pub struct UploadAvatarReplaceScenario;

impl Scenario for UploadAvatarReplaceScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<VisualTokenStr>;

    type Setup = ReplaceSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = login(ctx, &user_account(task.index)).await?;

        // 第一次上传, 记录旧头像 key
        let form = file_form("old.png", "image/png", PNG_BYTES.clone())?;
        let resp: SucR<VisualTokenStr> = ctx
            .client
            .request(reqwest::Method::PUT, "/user/avatar")
            .header("Authorization", &session.auth_header())
            .multipart(form)
            .send()
            .await?
            .json()
            .await?;

        Ok(ReplaceSetup {
            session,
            old_key: resp.data.0.file_id,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let form = file_form("new.png", "image/png", PNG_BYTES.clone())?;
        ctx.client
            .request(reqwest::Method::PUT, "/user/avatar")
            .header("Authorization", &setup.session.auth_header())
            .multipart(form)
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let new_key = &output.data.0.file_id;
        let distinct = setup.old_key != *new_key;

        let db_file = auth::user::Entity::find()
            .filter(auth::user::Column::Id.eq(setup.session.user_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .and_then(|u| u.avatar_file_id);
        let db_ok = db_file.as_deref() == Some(new_key.as_str());

        // S3 回查: 新对象存在且内容正确, 旧对象已被删除
        let new_exists = ctx.s3_bytes(new_key).await.as_deref() == Some(PNG_BYTES.as_slice());
        let old_gone = ctx.s3_missing(&setup.old_key).await;

        Ok(distinct && db_ok && new_exists && old_gone)
    }
}

register_scenario!(UploadAvatarReplaceScenario);

/// 非法文件类型: 期望 400(文件校验失败).
#[derive(Default)]
pub struct UploadAvatarInvalidFileScenario;

impl Scenario for UploadAvatarInvalidFileScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &user_account(task.index)).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let form = file_form("evil.txt", "text/plain", b"not an image".to_vec())?;
        ctx.client
            .request(reqwest::Method::PUT, "/user/avatar")
            .header("Authorization", &setup.auth_header())
            .multipart(form)
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.code == 400)
    }
}

register_scenario!(UploadAvatarInvalidFileScenario);

/// 未携带认证头: 期望 401.
#[derive(Default)]
pub struct UploadAvatarUnauthorizedScenario;

impl Scenario for UploadAvatarUnauthorizedScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::PUT, "/user/avatar")
            .json_unwrap(&json!({}))
            .send()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.code == 401)
    }
}

register_scenario!(UploadAvatarUnauthorizedScenario);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_fixture_has_valid_signature() {
        assert_eq!(
            &PNG_BYTES[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }
}
