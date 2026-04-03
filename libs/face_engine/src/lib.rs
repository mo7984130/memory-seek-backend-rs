use image::DynamicImage;
use thiserror::Error;
use tracing::info;

mod aligner;
mod base;
mod detector;
mod lazy_engine;
mod recognizer;
mod types;

pub use aligner::FaceAligner;
pub use detector::Detector;
pub use lazy_engine::LazyFaceEngine;
pub use recognizer::Recognizer;
pub use types::{BBox, FaceDetection, Point};

#[derive(Error, Debug)]
pub enum FaceEngineError {
    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
    #[error("ONNX Runtime error: {0}")]
    OrtError(String),
    #[error("No face detected")]
    NoFaceDetected,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Invalid output: {0}")]
    InvalidOutput(String),
    #[error("Face alignment failed")]
    AlignmentFailed,
    #[error("Model not loaded")]
    ModelNotLoaded,
    #[error("Face engine is disabled")]
    EngineDisabled,
}

pub struct FaceEngine {
    detector: Detector,
    recognizer: Recognizer,
}

impl FaceEngine {
    pub fn new(det_model_path: &str, rec_model_path: &str) -> Result<Self, FaceEngineError> {
        info!("初始化人脸识别模型...");
        info!("Detector 模型路径: {}", det_model_path);
        info!("Recognizer 模型路径: {}", rec_model_path);

        let detector = Detector::new(det_model_path)?;
        let recognizer = Recognizer::new(rec_model_path)?;

        detector.warmup()?;
        recognizer.warmup()?;

        info!("FaceEngine initialized successfully");

        Ok(Self { detector, recognizer })
    }

    pub fn detect_faces(&self, image_bytes: &[u8]) -> Result<Vec<FaceDetection>, FaceEngineError> {
        self.detector.detect(image_bytes)
    }

    pub fn extract_embedding(&self, aligned_face: &DynamicImage) -> Result<[f32; 512], FaceEngineError> {
        self.recognizer.extract(aligned_face)
    }

    pub fn detect_and_extract(
        &self,
        image_bytes: &[u8],
    ) -> Result<Vec<(FaceDetection, [f32; 512])>, FaceEngineError> {
        let detections = self.detect_faces(image_bytes)?;

        let mut results = Vec::new();

        for detection in detections {
            let aligned = FaceAligner::align(image_bytes, &detection.landmarks)?;
            let embedding = self.extract_embedding(&aligned)?;
            results.push((detection, embedding));
        }

        Ok(results)
    }
}
