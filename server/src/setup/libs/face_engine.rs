use std::sync::{Arc, Mutex};

use insight_face_rs::FaceEngine;

use crate::config::AppConfig;

pub fn init(cfg: &AppConfig) -> Arc<Mutex<FaceEngine>> {
    let engine = FaceEngine::new(
        &cfg.face_engine.detect_model_path,
        &cfg.face_engine.recognize_model_path,
    )
    .expect("fail to init face engine");
    Arc::new(Mutex::new(engine))
}
