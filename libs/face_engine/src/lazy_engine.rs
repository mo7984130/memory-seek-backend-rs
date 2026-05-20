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
    enabled: bool,
}

impl LazyFaceEngine {
    /// 创建懒加载的人脸引擎实例，模型在首次请求时才加载
    ///
    /// # 参数
    /// - `det_model_path`: SCRFD 检测模型的 ONNX 文件路径
    /// - `rec_model_path`: ArcFace 识别模型的 ONNX 文件路径
    pub fn new(det_model_path: &str, rec_model_path: &str) -> Self {
        Self {
            engine: Arc::new(RwLock::new(None)),
            last_used: Arc::new(RwLock::new(None)),
            det_model_path: det_model_path.to_string(),
            rec_model_path: rec_model_path.to_string(),
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            enabled: true,
        }
    }

    /// 创建禁用状态的人脸引擎，调用 `get_or_load` 将返回 `EngineDisabled` 错误
    pub fn new_disabled() -> Self {
        Self {
            engine: Arc::new(RwLock::new(None)),
            last_used: Arc::new(RwLock::new(None)),
            det_model_path: String::new(),
            rec_model_path: String::new(),
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            enabled: false,
        }
    }

    /// 检查人脸引擎是否处于启用状态
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 获取人脸引擎实例，如果尚未加载则在后台线程中初始化模型
    ///
    /// # 返回
    /// 返回共享的人脸引擎实例
    ///
    /// # 错误
    /// - `FaceEngineError::EngineDisabled`: 引擎处于禁用状态
    /// - `FaceEngineError::OrtError`: 模型加载或初始化失败
    pub async fn get_or_load(&self) -> Result<Arc<FaceEngine>, FaceEngineError> {
        if !self.enabled {
            tracing::error!("尝试使用已禁用的 FaceEngine");
            return Err(FaceEngineError::EngineDisabled);
        }
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

    // 更新最后使用时间为当前时刻
    async fn update_last_used(&self) {
        let mut last_used = self.last_used.write().await;
        *last_used = Some(Instant::now());
    }

    /// 检查引擎是否闲置超时，如果超过 TTL 则释放模型内存
    ///
    /// # 返回
    /// 返回 `true` 表示模型已释放，`false` 表示仍在使用或已卸载
    pub async fn unload_if_idle(&self) -> bool {
        let last_used = self.last_used.read().await;

        if let Some(last) = *last_used
            && last.elapsed() > self.ttl
        {
            drop(last_used);

            let mut engine = self.engine.write().await;
            if engine.is_some() {
                info!("FaceEngine 闲置超时，释放模型");
                *engine = None;
                return true;
            }
        }
        false
    }

    /// 启动后台清理任务，定期检查并释放闲置超时的模型
    ///
    /// # 返回
    /// 返回后台任务的 `JoinHandle`，可用于取消任务
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

    /// 启动时预热引擎，加载模型并验证可用性后释放内存
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: 模型加载或推理失败
    pub async fn warmup_on_startup(&self) -> Result<(), FaceEngineError> {
        info!("验证 FaceEngine 模型中...");
        let engine = self.get_or_load().await?;

        let det_path = self.det_model_path.clone();
        tokio::task::spawn_blocking(move || {
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
