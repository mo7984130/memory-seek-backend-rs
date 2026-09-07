use std::{sync::Arc, time::Duration};

use insight_face_rs::{FaceEngine, FaceEngineConfig};
use serde::Deserialize;
use tracing::{debug, info};

use crate::{config::AppConfig, setup::AppSetup};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub detect_model_path: String,
    pub recognize_model_path: String,
}

#[common::register_async(
    slice = crate::setup::libs::APP_LIBS,
    ty = crate::setup::InitFn,
)]
pub async fn init(config: &AppConfig, setup: &mut AppSetup) -> common::Result<()> {
    debug!("初始化 FaceEngine lib");
    let config = &config.face_engine;
    let config = FaceEngineConfig::new(
        config.detect_model_path.clone(),
        config.recognize_model_path.clone(),
        Duration::from_secs(60),
    );
    let engine = FaceEngine::new(&config).expect("fail to init face engine");
    engine.unload().expect("unload engine fail");

    let engine = Arc::new(engine);
    FaceEngine::start_reaper_thread(&engine);

    setup.registry.insert(engine);

    info!("初始化 FaceEngine lib 成功");
    Ok(())
}
