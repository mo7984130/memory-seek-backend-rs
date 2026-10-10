//! 影像上传:`POST /visual`。

use memory_seek_api::{Method, VisualIds, api};
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode, TeardownMode},
};
use sea_orm::EntityTrait;
use types_visual::dto::visual::VisualView;
use types_visual::visual as visual_entity;
use types_visual::visual::{VisualId, VisualKind};

use crate::context::Context;
use crate::error::{E2eError, status};

use super::{Session, blake3_hex, session, token_matches, unique_png, unique_tag};

/// 上传前置:登录 + 本次任务唯一影像字节。
/// `uploaded_id` 供 setup 阶段就上传的场景(如重复上传)收尾时删除。
#[derive(Default)]
pub struct UploadSetup {
    pub session: Session,
    pub bytes: Vec<u8>,
    pub uploaded_id: Option<VisualId>,
}

async fn prepare(ctx: &Context, task: &TaskIndex) -> Result<UploadSetup, E2eError> {
    let session = session(ctx, task.index).await?;
    Ok(UploadSetup {
        session,
        bytes: unique_png(&unique_tag(task)),
        uploaded_id: None,
    })
}

/// 上传成功: 响应字段、库中记录与 S3 对象三者一致, 且三档 token 绑定浏览者。
#[derive(Default)]
pub struct UploadVisualScenario;

impl Scenario for UploadVisualScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = VisualView;

    type Setup = UploadSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        prepare(ctx, task).await
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        super::upload(&setup.session, setup.bytes.clone()).await
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let view = output;
        let Some(row) = visual_entity::Entity::find_by_id(view.id)
            .one(&ctx.db)
            .await
            .unwrap()
        else {
            return Ok(false);
        };

        // 落库字段与请求内容一致。
        // 上传接口只收原始字节、无客户端文件名, 服务端以生成的 uuid 作为 name,
        // 并写入 file_id = `visuals/YYYY/MM/DD/<uuid>.png`。
        let name_ok = row.name.len() == 36
            && row.name.matches('-').count() == 4
            && row.file_id.starts_with("visuals/")
            && row.file_id.ends_with(&format!("{}.png", row.name));
        let db_ok = row.user_id == setup.session.user_id
            && name_ok
            && row.kind == VisualKind::Image
            && row.hash == blake3_hex(&setup.bytes)
            && row.size as u64 == setup.bytes.len() as u64
            && row.width == 1
            && row.height == 1;

        // 响应与库一致
        let view_ok = view.user_id == setup.session.user_id
            && view.name == row.name
            && view.width == 1
            && view.height == 1;

        // 三档 token 均绑定到 file_id 与当前浏览者
        let viewer = setup.session.user_id;
        let tokens_ok = token_matches(view.thumbnail_token.as_ref(), &row.file_id, viewer)
            && token_matches(view.preview_token.as_ref(), &row.file_id, viewer)
            && token_matches(view.original_token.as_ref(), &row.file_id, viewer);

        // S3 对象已落盘且内容与上传字节一致
        let s3_ok = ctx.s3_bytes(&row.file_id).await.as_deref() == Some(setup.bytes.as_slice());

        Ok(db_ok && view_ok && tokens_ok && s3_ok)
    }

    /// 收尾: 删除本轮上传的影像(库记录 + S3 对象由服务端异步清理)。
    /// 压测长跑下每轮新增一张, 不回收会写爆库与对象存储
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let Some(Ok(view)) = result else {
            // run 失败 = 服务端未接受上传, 无产物可删
            return Ok(());
        };
        crate::cleanup::idempotent(
            api::visual::delete_visuals(
                setup.session.client(),
                VisualIds::new(vec![view.id]).expect("单元素影像列表恒合法"),
            )
            .await,
        )
    }
}

register_scenario!(UploadVisualScenario);

/// 重复上传同一内容: 期望 400(命中哈希去重)。
#[derive(Default)]
pub struct UploadDuplicateVisualScenario;

impl Scenario for UploadDuplicateVisualScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = UploadSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let mut setup = prepare(ctx, task).await?;
        // 先成功上传一次, 为 run 制造哈希命中; id 留给收尾删除
        let view = super::upload(&setup.session, setup.bytes.clone()).await?;
        setup.uploaded_id = Some(view.id);
        Ok(setup)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        status(
            api::visual::upload_visual(
                setup.session.client(),
                setup.bytes.clone(),
                Some("image/png".to_owned()),
                None,
            )
            .await,
        )
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 400)
    }

    /// 收尾: setup 阶段为制造哈希命中上传的那张影像(每任务一张, 默认 Task 粒度)
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let Some(visual_id) = setup.uploaded_id else {
            return Ok(());
        };
        crate::cleanup::idempotent(
            api::visual::delete_visuals(
                setup.session.client(),
                VisualIds::new(vec![visual_id]).expect("单元素影像列表恒合法"),
            )
            .await,
        )
    }
}

register_scenario!(UploadDuplicateVisualScenario);

/// 非法文件: 期望 400(文件校验失败)。
#[derive(Default)]
pub struct UploadInvalidFileScenario;

impl Scenario for UploadInvalidFileScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(session(ctx, task.index).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        status(
            api::visual::upload_visual(
                setup.client(),
                b"not an image".to_vec(),
                Some("text/plain".to_owned()),
                None,
            )
            .await,
        )
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 400)
    }
}

register_scenario!(UploadInvalidFileScenario);

/// 未携带认证头: 期望 401。
#[derive(Default)]
pub struct UploadUnauthorizedScenario;

impl Scenario for UploadUnauthorizedScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = u16;

    type Setup = ();

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        _setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let response = ctx
            .api_client()
            .raw(Method::POST, "/visual")
            .no_auth()
            .bytes(unique_png(&unique_tag(task)), Some("image/png".to_owned()))
            .send()
            .await?;
        Ok(response.status.as_u16())
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        _setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(*output == 401)
    }
}

register_scenario!(UploadUnauthorizedScenario);
