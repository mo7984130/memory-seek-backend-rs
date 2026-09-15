//! 相册:`/photo/collections`。

use common::axum::{ErrR, SucR};
use memseek_test::{
    TaskIndex,
    ctxlibs::http_client::HttpError,
    register_scenario,
    scenario::{Scenario, SetupMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use types::photo::collection as collection_entity;
use types::photo::collection::CollectionId;
use types::photo::collection_photo as collection_photo_entity;
use types::photo::dto::collection::{CollectionPhotoAddBatchResult, CollectionView};
use types::photo::photo::PhotoId;

use crate::context::Context;

use super::{Session, seed_photo, session};

/// 按 API 创建收藏夹, 返回视图。
async fn create_collection(
    ctx: &Context,
    session: &Session,
    name: String,
) -> Result<SucR<CollectionView>, HttpError> {
    ctx.client
        .request(reqwest::Method::POST, "/photo/collections")
        .header("Authorization", &session.auth_header())
        .json_unwrap(&json!({ "name": name, "description": "e2e" }))
        .send_checked()
        .await?
        .json::<SucR<CollectionView>>()
        .await
        .map_err(HttpError::from)
}

/// 创建收藏夹前置:登录 + 建立前置收藏夹(需要时)。
/// id 以 i64 保存, 便于 `Default`(强类型 ID 不实现 `Default`)。
#[derive(Default)]
pub struct CollectionSetup {
    pub session: Session,
    pub collection_id: i64,
}

async fn create_setup(ctx: &Context, task: &TaskIndex) -> Result<CollectionSetup, HttpError> {
    let session = session(ctx, task.index).await?;
    let name = format!("e2e_album_{}", task.index);
    let view = create_collection(ctx, &session, name).await?.data;
    Ok(CollectionSetup {
        session,
        collection_id: view.id.0,
    })
}

/// 创建收藏夹: 落库记录归属当前用户且名称一致。
#[derive(Default)]
pub struct CreateCollectionScenario;

impl Scenario for CreateCollectionScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CollectionView>;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        session(ctx, task.index).await
    }

    async fn run(
        ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        create_collection(ctx, setup, format!("e2e_album_{}", task.index)).await
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let Some(row) = collection_entity::Entity::find_by_id(output.data.id)
            .one(&ctx.db)
            .await
            .unwrap()
        else {
            return Ok(false);
        };
        Ok(row.user_id == setup.user_id && row.name == output.data.name)
    }
}

register_scenario!(CreateCollectionScenario);

/// 空名称创建: 期望 400(参数校验失败)。
#[derive(Default)]
pub struct CreateCollectionInvalidNameScenario;

impl Scenario for CreateCollectionInvalidNameScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = ErrR;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        session(ctx, task.index).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::POST, "/photo/collections")
            .header("Authorization", &setup.auth_header())
            .json_unwrap(&json!({ "name": "" }))
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

register_scenario!(CreateCollectionInvalidNameScenario);

/// 收藏夹列表: 包含前置创建的收藏夹。
#[derive(Default)]
pub struct GetCollectionsScenario;

impl Scenario for GetCollectionsScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<Vec<CollectionView>>;

    type Setup = CollectionSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        create_setup(ctx, task).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(reqwest::Method::GET, "/photo/collections")
            .header("Authorization", &setup.session.auth_header())
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.data.iter().any(|c| c.id.0 == setup.collection_id))
    }
}

register_scenario!(GetCollectionsScenario);

/// 更新收藏夹: 名称落库生效。
#[derive(Default)]
pub struct UpdateCollectionScenario;

impl Scenario for UpdateCollectionScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = CollectionSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        create_setup(ctx, task).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(
                reqwest::Method::PATCH,
                &format!("/photo/collections/{}", setup.collection_id),
            )
            .header("Authorization", &setup.session.auth_header())
            .json_unwrap(&json!({ "name": "e2e_renamed" }))
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let row = collection_entity::Entity::find_by_id(CollectionId(setup.collection_id))
            .one(&ctx.db)
            .await
            .unwrap();
        Ok(row.is_some_and(|r| r.name == "e2e_renamed"))
    }
}

register_scenario!(UpdateCollectionScenario);

/// 删除收藏夹: 记录消失。
#[derive(Default)]
pub struct DeleteCollectionScenario;

impl Scenario for DeleteCollectionScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<()>;

    type Setup = CollectionSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        create_setup(ctx, task).await
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(
                reqwest::Method::DELETE,
                &format!("/photo/collections/{}", setup.collection_id),
            )
            .header("Authorization", &setup.session.auth_header())
            .send_checked()
            .await?
            .json::<Self::Output>()
            .await
            .map_err(HttpError::from)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let gone = collection_entity::Entity::find_by_id(CollectionId(setup.collection_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(DeleteCollectionScenario);

/// 相册添加照片前置:登录 + 种子照片 + 前置收藏夹。
/// id 以 i64 保存, 便于 `Default`(强类型 ID 不实现 `Default`)。
#[derive(Default)]
pub struct AddPhotosSetup {
    pub session: Session,
    pub collection_id: i64,
    pub photo_id: i64,
}

/// 相册批量添加照片: 关联记录落库, 计数与新增数正确。
#[derive(Default)]
pub struct AddPhotosToCollectionScenario;

impl Scenario for AddPhotosToCollectionScenario {
    type Ctx = Context;

    type Error = HttpError;

    type Output = SucR<CollectionPhotoAddBatchResult>;

    type Setup = AddPhotosSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let photo = seed_photo(ctx, 5).await.ok_or_else(super::seed_missing)?;
        let view = create_collection(ctx, &session, format!("e2e_album_add_{}", task.index))
            .await?
            .data;
        Ok(AddPhotosSetup {
            session,
            collection_id: view.id.0,
            photo_id: photo.id.0,
        })
    }

    async fn run(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        ctx.client
            .request(
                reqwest::Method::POST,
                &format!("/photo/collections/{}/photos", setup.collection_id),
            )
            .header("Authorization", &setup.session.auth_header())
            .json_unwrap(&json!({ "photoIds": [setup.photo_id] }))
            .send_checked()
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
        let linked = collection_photo_entity::Entity::find()
            .filter(
                collection_photo_entity::Column::CollectionId.eq(CollectionId(setup.collection_id)),
            )
            .filter(collection_photo_entity::Column::PhotoId.eq(PhotoId(setup.photo_id)))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some();
        let count_ok = output.data.new_photo_count == 1;
        let collection_ok =
            collection_entity::Entity::find_by_id(CollectionId(setup.collection_id))
                .one(&ctx.db)
                .await
                .unwrap()
                .is_some_and(|c| c.photo_count == 1);
        Ok(linked && count_ok && collection_ok)
    }
}

register_scenario!(AddPhotosToCollectionScenario);
