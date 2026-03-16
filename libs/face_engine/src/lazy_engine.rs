use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

use crate::{FaceEngine, FaceEngineError};

const DEFAULT_TTL_SECS: u64 = 600;
const CLEANUP_INTERVAL_SECS: u64 = 60;

pub struct LazyFaceEngine {
    engine: Arc<RwLock<Option<Arc<FaceEngine>>>>,
    last_used: Arc<RwLock<Option<Instant>>>,
    det_model_path: String,
    rec_model_path: String,
    ttl: Duration,
}

impl LazyFaceEngine {
    pub fn new(det_model_path: &str, rec_model_path: &str) -> Self {
        Self {
            engine: Arc::new(RwLock::new(None)),
            last_used: Arc::new(RwLock::new(None)),
            det_model_path: det_model_path.to_string(),
            rec_model_path: rec_model_path.to_string(),
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
        }
    }

    pub async fn get_or_load(&self) -> Result<Arc<FaceEngine>, FaceEngineError> {
        {
            let engine_read = self.engine.read().await;
            if let Some(engine) = engine_read.as_ref() {
                self.update_last_used().await;
                return Ok(engine.clone());
            }
        }

        let mut engine_write = self.engine.write().await;

        if let Some(engine) = engine_write.as_ref() {
            self.update_last_used().await;
            return Ok(engine.clone());
        }

        let det_path = self.det_model_path.clone();
        let rec_path = self.rec_model_path.clone();

        let engine = tokio::task::spawn_blocking(move || FaceEngine::new(&det_path, &rec_path))
            .await
            .map_err(|e| FaceEngineError::OrtError(e.to_string()))??;

        let engine = Arc::new(engine);
        *engine_write = Some(engine.clone());
        self.update_last_used().await;

        info!("FaceEngine 懒加载完成");
        Ok(engine)
    }

    async fn update_last_used(&self) {
        let mut last_used = self.last_used.write().await;
        *last_used = Some(Instant::now());
    }

    pub async fn unload_if_idle(&self) -> bool {
        let last_used = self.last_used.read().await;

        if let Some(last) = *last_used {
            if last.elapsed() > self.ttl {
                drop(last_used);

                let mut engine = self.engine.write().await;
                if engine.is_some() {
                    info!("FaceEngine 闲置超时，释放模型");
                    *engine = None;
                    return true;
                }
            }
        }
        false
    }

    pub fn start_cleanup_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));

            loop {
                interval.tick().await;
                self.unload_if_idle().await;
            }
        })
    }

    pub async fn warmup_on_startup(&self) -> Result<(), FaceEngineError> {
        info!("验证 FaceEngine 模型中...");
        let engine = self.get_or_load().await?;

        let det_path = self.det_model_path.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _ = &engine;
            let _ = &det_path;
        })
        .await
        .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

        info!("FaceEngine 模型验证通过");

        let mut engine_guard = self.engine.write().await;
        if engine_guard.is_some() {
            info!("验证完成，释放模型等待首次请求");
            *engine_guard = None;
        }

        let mut last_used = self.last_used.write().await;
        *last_used = None;

        Ok(())
    }
}
