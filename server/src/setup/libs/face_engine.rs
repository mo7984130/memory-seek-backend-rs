use std::{sync::Arc, time::Duration};

use insight_face_rs::{FaceEngine, FaceEngineConfig};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub detect_model_path: String,
    pub recognize_model_path: String,
}

/// 根据配置初始化人脸识别引擎.
pub fn init(cfg: &Config) -> Arc<FaceEngine> {
    info!("初始化人脸识别模型");
    let config = FaceEngineConfig::new(
        cfg.detect_model_path.clone(),
        cfg.recognize_model_path.clone(),
        Duration::from_secs(60),
    );
    let engine = FaceEngine::new(&config).expect("fail to init face engine");
    engine.unload().expect("unload engine fail");

    let engine = Arc::new(engine);
    FaceEngine::start_reaper_thread(&engine);

    info!("人脸识别模型初始化成功");

    engine
}
