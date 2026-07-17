use std::sync::{Arc, Mutex};

use insight_face_rs::FaceEngine;
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub detect_model_path: String,
    pub recognize_model_path: String,
}

pub fn init(cfg: &Config) -> Arc<Mutex<FaceEngine>> {
    info!("初始化人脸识别模型");
    let engine = FaceEngine::new(&cfg.detect_model_path, &cfg.recognize_model_path)
        .expect("fail to init face engine");
    info!("人脸识别模型初始化成功");
    Arc::new(Mutex::new(engine))
}
