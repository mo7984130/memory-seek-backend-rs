pub mod bases;
pub mod domains;
pub mod libs;

use crate::config::AppConfig;
use crate::state::AppState;
use axum::Router;
use std::sync::Arc;

pub struct AppSetup {
    pub state: Arc<AppState>,
    pub public_router: Router<Arc<AppState>>,
    pub protected_router: Router<Arc<AppState>>,
}

impl AppSetup {
    pub async fn init(cfg: &AppConfig) -> anyhow::Result<Self> {
        // 1. 初始化基础设施
        let bases = bases::AppBasesInit::init(cfg).await?;

        // 2. 初始化库
        let libs = libs::AppLibsInit::init(cfg).await?;

        // 3. 构建 AppState
        let state = Arc::new(AppState::from((bases, libs)));

        // 4. 注册业务模块
        let (public_router, protected_router) = domains::AppDomains::init(&state, cfg);

        Ok(Self {
            state,
            public_router,
            protected_router,
        })
    }
}
