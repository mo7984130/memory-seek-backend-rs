//! 通用事务步骤管道
//!
//! 提供 [`Step`] trait 与 [`StepPipeline`]:在单个数据库事务内严格串行执行一组
//! 自包含的清理/变更步骤。每个步骤声明其允许调用的 Mapper 白名单([`Step::owns`]),
//! 由 `tools/step_boundary_check` 静态检查"不得越权操作其它表"。
//!
//! 步骤采用**定义即注册**:每个步骤在声明处经 `linkme` 分布式切片注册为
//! `&'static dyn Step<Ctx>` 元素,调用方将收集到的列表交给
//! [`StepPipeline::from_slice_stable`] 按 [`Step::is_final`] 稳定排序(非 final 步骤
//! 保持收集顺序、final 步骤置末),从而无需维护集中的注册列表。

use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};

use crate::Result;

/// 事务管道中的单个步骤
///
/// 约束:
/// - **自包含**:只操作 [`Step::owns`] 声明白名单内的 Mapper,关联数据由 mapper 内部自给;
/// - **串行**:管道依次调用 [`Step::execute`],任一步失败则整个事务回滚;
/// - **顺序无关**:步骤之间不依赖执行先后;[`Step::is_final`] 的步骤恒在最后执行。
#[async_trait]
pub trait Step<Ctx>: Send + Sync {
    /// 步骤名(日志 / 追踪 / 度量用)
    fn name(&self) -> &'static str;

    /// 本步骤允许调用的 Mapper 类型名白名单,供 `step_boundary_check` 校验
    fn owns(&self) -> &'static [&'static str];

    /// 是否必须在最后执行。默认 `false`;置 `true` 的步骤(如受外键约束的"删除主表")
    /// 由管道收集后移到末尾。
    fn is_final(&self) -> bool {
        false
    }

    /// 在事务内执行本步骤
    async fn execute(&self, txn: &DatabaseTransaction, ctx: &mut Ctx) -> Result<()>;
}

/// 按序执行一组步骤的事务管道
pub struct StepPipeline<Ctx: 'static> {
    steps: &'static [&'static dyn Step<Ctx>],
}

impl<Ctx: Send + 'static> StepPipeline<Ctx> {
    /// 从步骤引用列表构建管道,按 [`Step::is_final`] **稳定排序**(非 final 保持原顺序、
    /// final 步骤置末),并以 `&'static` 数组持有。
    ///
    /// 配合 `linkme` 分布式切片等"定义即注册"机制使用:调用方传入收集到的步骤列表,
    /// 本方法负责排序与生命周期提升,消除重复样板。
    pub fn from_slice_stable(mut steps: Vec<&'static dyn Step<Ctx>>) -> Self {
        steps.sort_by_key(|step| step.is_final());
        let steps: &'static [&'static dyn Step<Ctx>] = Box::leak(steps.into_boxed_slice());
        Self::new(steps)
    }

    /// 创建管道。数组元素的顺序即执行顺序。
    pub fn new(steps: &'static [&'static dyn Step<Ctx>]) -> Self {
        Self { steps }
    }

    // todo mutlirun
    /// 在单个事务内串行执行全部步骤,任一步失败则整体回滚
    pub async fn run(&self, db: &DatabaseConnection, ctx: &mut Ctx) -> Result<()> {
        let txn = db.begin().await?;
        let res: Result<()> = async {
            for step in self.steps {
                step.execute(&txn, ctx).await?;
            }
            Ok(())
        }
        .await;

        match res {
            Ok(()) => {
                txn.commit().await?;
                Ok(())
            }
            Err(e) => {
                txn.rollback().await.ok();
                Err(e)
            }
        }
    }
}
