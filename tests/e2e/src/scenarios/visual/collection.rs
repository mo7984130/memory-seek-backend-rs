//! 相册:`/visual/collections`。

use memory_seek_api::{
    CollectionCreateParam, CollectionUpdateParam, CollectionVisualAddBatchParam, VisualIds, api,
};
use memseek_test::{
    TaskIndex, register_scenario,
    scenario::{Scenario, SetupMode, TeardownMode},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use types_visual::collection as collection_entity;
use types_visual::collection::CollectionId;
use types_visual::collection_visual as collection_visual_entity;
use types_visual::dto::collection::{CollectionView, CollectionVisualAddBatchResult};
use types_visual::visual::VisualId;

use crate::context::Context;
use crate::error::{E2eError, status};

use super::{Session, seed_visual, session};

/// 按 API 创建收藏夹, 返回视图。
async fn create_collection(session: &Session, name: String) -> Result<CollectionView, E2eError> {
    let param = CollectionCreateParam {
        name,
        description: Some("e2e".to_owned()),
    };
    Ok(api::visual::collection::create_collection(session.client(), &param).await?)
}

/// 幂等删除收藏夹(收尾复用)。
async fn delete_collection(session: &Session, id: CollectionId) -> Result<(), E2eError> {
    crate::cleanup::idempotent(
        api::visual::collection::delete_collection(session.client(), id).await,
    )
}

/// 创建收藏夹前置:登录 + 建立前置收藏夹(需要时)。
pub struct CollectionSetup {
    pub session: Session,
    pub collection_id: CollectionId,
}

impl Default for CollectionSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            collection_id: CollectionId(0),
        }
    }
}

async fn create_setup(ctx: &Context, task: &TaskIndex) -> Result<CollectionSetup, E2eError> {
    let session = session(ctx, task.index).await?;
    let name = format!("e2e_album_{}", task.index);
    let view = create_collection(&session, name).await?;
    Ok(CollectionSetup {
        session,
        collection_id: view.id,
    })
}

/// 创建收藏夹: 落库记录归属当前用户且名称一致。
#[derive(Default)]
pub struct CreateCollectionScenario;

impl Scenario for CreateCollectionScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = CollectionView;

    type Setup = Session;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        Ok(session(ctx, task.index).await?)
    }

    async fn run(
        _ctx: &Self::Ctx,
        task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        create_collection(setup, format!("e2e_album_{}", task.index)).await
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let Some(row) = collection_entity::Entity::find_by_id(output.id)
            .one(&ctx.db)
            .await
            .unwrap()
        else {
            return Ok(false);
        };
        Ok(row.user_id == setup.user_id && row.name == output.name)
    }

    /// 收尾: 删除本轮创建的收藏夹(长跑下每轮新增一个, 不回收会写爆库)
    const TEARDOWN_MODE: TeardownMode = TeardownMode::Round;

    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        let Some(Ok(view)) = result else {
            return Ok(());
        };
        delete_collection(setup, view.id).await
    }
}

register_scenario!(CreateCollectionScenario);

/// 空名称创建: 期望 400(参数校验失败)。
#[derive(Default)]
pub struct CreateCollectionInvalidNameScenario;

impl Scenario for CreateCollectionInvalidNameScenario {
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
        let param = CollectionCreateParam {
            name: String::new(),
            description: None,
        };
        status(api::visual::collection::create_collection(setup.client(), &param).await)
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

register_scenario!(CreateCollectionInvalidNameScenario);

/// 收藏夹列表: 包含前置创建的收藏夹。
#[derive(Default)]
pub struct GetCollectionsScenario;

impl Scenario for GetCollectionsScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = Vec<CollectionView>;

    type Setup = CollectionSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        create_setup(ctx, task).await
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(api::visual::collection::get_collection_list(setup.session.client()).await?)
    }

    async fn validate(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        Ok(output.iter().any(|c| c.id == setup.collection_id))
    }

    /// 收尾: 删除 setup 阶段创建的前置收藏夹
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        delete_collection(&setup.session, setup.collection_id).await
    }
}

register_scenario!(GetCollectionsScenario);

/// 更新收藏夹: 名称落库生效。
#[derive(Default)]
pub struct UpdateCollectionScenario;

impl Scenario for UpdateCollectionScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = CollectionSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        create_setup(ctx, task).await
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let param = CollectionUpdateParam {
            name: Some("e2e_renamed".to_owned()),
            description: None,
        };
        Ok(api::visual::collection::update_collection(
            setup.session.client(),
            setup.collection_id,
            &param,
        )
        .await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let row = collection_entity::Entity::find_by_id(setup.collection_id)
            .one(&ctx.db)
            .await
            .unwrap();
        Ok(row.is_some_and(|r| r.name == "e2e_renamed"))
    }

    /// 收尾: 删除 setup 阶段创建的前置收藏夹
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        delete_collection(&setup.session, setup.collection_id).await
    }
}

register_scenario!(UpdateCollectionScenario);

/// 删除收藏夹: 记录消失。
#[derive(Default)]
pub struct DeleteCollectionScenario;

impl Scenario for DeleteCollectionScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = ();

    type Setup = CollectionSetup;

    const SETUP_MODE: SetupMode = SetupMode::Round;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        create_setup(ctx, task).await
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        Ok(
            api::visual::collection::delete_collection(setup.session.client(), setup.collection_id)
                .await?,
        )
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let gone = collection_entity::Entity::find_by_id(setup.collection_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none();
        Ok(gone)
    }
}

register_scenario!(DeleteCollectionScenario);

/// 相册添加影像前置:登录 + 种子影像 + 前置收藏夹。
pub struct AddVisualsSetup {
    pub session: Session,
    pub collection_id: CollectionId,
    pub visual_id: VisualId,
}

impl Default for AddVisualsSetup {
    fn default() -> Self {
        Self {
            session: Session::default(),
            collection_id: CollectionId(0),
            visual_id: VisualId(0),
        }
    }
}

/// 相册批量添加影像: 关联记录落库, 计数与新增数正确。
#[derive(Default)]
pub struct AddVisualsToCollectionScenario;

impl Scenario for AddVisualsToCollectionScenario {
    type Ctx = Context;

    type Error = E2eError;

    type Output = CollectionVisualAddBatchResult;

    type Setup = AddVisualsSetup;

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        let session = session(ctx, task.index).await?;
        let visual = seed_visual(ctx, 5).await.ok_or_else(super::seed_missing)?;
        let view = create_collection(&session, format!("e2e_album_add_{}", task.index)).await?;
        Ok(AddVisualsSetup {
            session,
            collection_id: view.id,
            visual_id: visual.id,
        })
    }

    async fn run(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
    ) -> Result<Self::Output, Self::Error> {
        let param = CollectionVisualAddBatchParam {
            visual_ids: VisualIds::new(vec![setup.visual_id]).expect("单元素影像列表恒合法"),
        };
        Ok(api::visual::collection::add_visuals_to_collection(
            setup.session.client(),
            setup.collection_id,
            &param,
        )
        .await?)
    }

    async fn validate(
        ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        output: &Self::Output,
    ) -> Result<bool, Self::Error> {
        let linked = collection_visual_entity::Entity::find()
            .filter(collection_visual_entity::Column::CollectionId.eq(setup.collection_id))
            .filter(collection_visual_entity::Column::VisualId.eq(setup.visual_id))
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some();
        let count_ok = output.new_visual_count == 1;
        let collection_ok = collection_entity::Entity::find_by_id(setup.collection_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some_and(|c| c.visual_count == 1);
        Ok(linked && count_ok && collection_ok)
    }

    /// 收尾: 删除 setup 阶段创建的前置收藏夹(关联记录由服务端一并清理)
    async fn teardown(
        _ctx: &Self::Ctx,
        _task: &TaskIndex,
        setup: &Self::Setup,
        _result: Option<Result<&Self::Output, &Self::Error>>,
    ) -> Result<(), Self::Error> {
        delete_collection(&setup.session, setup.collection_id).await
    }
}

register_scenario!(AddVisualsToCollectionScenario);
