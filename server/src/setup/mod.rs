pub mod bases;
pub mod domains;
pub mod libs;

use common_core::{Result, TypeMap};
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use axum::Router;
use std::pin::Pin;

pub struct AppRouter {
    pub protected: Router,
    pub public: Router,
}

impl Default for AppRouter {
    /// 返回空的默认路由器
    fn default() -> Self {
        Self::new()
    }
}

impl AppRouter {
    /// 创建空的受保护与公开路由器
    pub fn new() -> Self {
        Self {
            protected: Router::new(),
            public: Router::new(),
        }
    }

    /// 合并入公开路由
    #[allow(unused)]
    pub fn add_public(&mut self, router: Router) {
        self.public = std::mem::take(&mut self.public).merge(router);
    }

    /// 合并入受保护路由
    #[allow(unused)]
    pub fn add_protected(&mut self, router: Router) {
        self.protected = std::mem::take(&mut self.protected).merge(router);
    }
}

type InitFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + 'a>>;

type InitFn = for<'a> fn(&'a AppConfig, &'a mut AppSetup) -> InitFuture<'a>;

/// 依次执行传入的初始化函数
#[inline]
async fn inits(config: &AppConfig, setup: &mut AppSetup, fns: &[InitFn]) -> Result<()> {
    for init in fns {
        init(config, setup).await?;
    }
    Ok(())
}

pub struct AppSetup {
    pub registry: TypeMap,
    pub router: AppRouter,
    pub cancel_token: CancellationToken,
}
impl AppSetup {
    /// 创建空的 `AppSetup`
    fn new() -> Self {
        Self {
            registry: TypeMap::new(),
            router: AppRouter::new(),
            cancel_token: CancellationToken::new(),
        }
    }

    /// 初始化基础设施, 外部库, 业务域和应用路由
    pub async fn init(config: &AppConfig) -> Result<Self> {
        let mut setup = Self::new();

        // 初始化基础设施
        inits(config, &mut setup, &bases::APP_BASES).await?;

        // 初始化库
        inits(config, &mut setup, &libs::APP_LIBS).await?;

        // 初始化领域
        inits(config, &mut setup, &domains::APP_DOMAINS_FIRST).await?;
        inits(config, &mut setup, &domains::APP_DOMAINS).await?;

        // 最后注册 metrics(需要 registry 与 router 均就绪)
        inits(config, &mut setup, &bases::APP_BASES_LAST).await?;

        Ok(setup)
    }
}
