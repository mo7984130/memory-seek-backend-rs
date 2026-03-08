use image::DynamicImage;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum FaceEngineError {
    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
    #[error("No face detected")]
    NoFaceDetected,
    #[error("Invalid input")]
    InvalidInput,
    #[error("Not implemented")]
    NotImplemented,
}

#[derive(Debug, Clone)]
pub struct FaceDetection {
    pub bbox: BBox,
    pub landmarks: [f32; 10],
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BBox {
    pub fn area(&self) -> f32 {
        self.w * self.h
    }
}

pub struct FaceEngine {
    _detector_path: String,
    _recognizer_path: String,
}

impl FaceEngine {
    pub fn new(det_model_path: &str, rec_model_path: &str) -> Result<Self, FaceEngineError> {
        info!("Face engine initialized (stub mode)");
        info!("Detector model: {}", det_model_path);
        info!("Recognizer model: {}", rec_model_path);
        Ok(Self {
            _detector_path: det_model_path.to_string(),
            _recognizer_path: rec_model_path.to_string(),
        })
    }

    pub fn detect_faces(&self, _image_bytes: &[u8]) -> Result<Vec<FaceDetection>, FaceEngineError> {
        Ok(vec![])
    }

    pub fn extract_embedding(&self, _aligned_face: &DynamicImage) -> Result<[f32; 512], FaceEngineError> {
        Ok([0.0f32; 512])
    }
}

pub struct FaceAligner;

impl FaceAligner {
    pub fn align(image_bytes: &[u8], _landmarks: &[f32; 10]) -> Result<DynamicImage, FaceEngineError> {
        let img = image::load_from_memory(image_bytes)?;
        let resized = img.resize_exact(112, 112, image::imageops::FilterType::Lanczos3);
        Ok(resized)
    }
}
