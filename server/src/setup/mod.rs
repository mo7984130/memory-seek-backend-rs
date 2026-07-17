pub mod bases;
pub mod domains;
pub mod libs;

use common::error::AppError;

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
    pub async fn init(cfg: &AppConfig) -> Result<Self, AppError> {
        // 1. 初始化基础设施
        let bases = bases::AppBasesInit::init(cfg).await?;

        // 2. 初始化库
        let libs = libs::AppLibsInit::init(cfg).await?;

        // 3. 初始化备份调度器
        #[cfg(feature = "backup")]
        let backup_scheduler = {
            let backup_config = cfg
                .backup
                .as_ref()
                .expect("启用 backup 功能时必须在配置中设置 backup 项");
            domains::backup::init(&bases.db, &libs.s3_client, backup_config).await?
        };

        // 4. 构建 AppState
        let state = Arc::new(AppState {
            db: bases.db,
            redis: bases.redis,
            token_cipher: libs.token_cipher,
            email_client: libs.email_client,
            #[cfg(feature = "s3")]
            s3_client: libs.s3_client,
            #[cfg(feature = "backup")]
            backup_scheduler,
            #[cfg(feature = "face-engine")]
            face_engine: libs.face_engine,
        });

        // 5. 注册业务模块
        let (public_router, protected_router) = domains::AppDomains::init(&state, cfg);

        Ok(Self {
            state,
            public_router,
            protected_router,
        })
    }
}
