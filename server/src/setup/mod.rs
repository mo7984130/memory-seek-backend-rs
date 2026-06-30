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

        // 3. 初始化备份调度器
        #[cfg(feature = "backup")]
        let backup_scheduler = if let Some(ref backup_config) = cfg.backup {
            if backup_config.enabled {
                let backup_state = Arc::new(backup::BackupState::new(
                    bases.db.clone(),
                    libs.s3_client.clone(),
                    backup_config.clone(),
                ));
                let scheduler = backup::BackupScheduler::new(backup_state)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                scheduler
                    .start()
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                Some(Arc::new(scheduler))
            } else {
                None
            }
        } else {
            None
        };

        // 4. 构建 AppState
        let state = Arc::new(AppState {
            db: bases.db,
            redis: bases.redis,
            token_cipher: libs.token_cipher,
            #[cfg(feature = "s3")]
            s3_client: libs.s3_client,
            #[cfg(feature = "backup")]
            backup_scheduler,
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
