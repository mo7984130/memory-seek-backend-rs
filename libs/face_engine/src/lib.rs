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
    /// 初始化人脸引擎，加载检测和识别模型并执行预热
    ///
    /// # 参数
    /// - `det_model_path`: SCRFD 检测模型的 ONNX 文件路径
    /// - `rec_model_path`: ArcFace 识别模型的 ONNX 文件路径
    ///
    /// # 返回
    /// 返回初始化完成的 `FaceEngine` 实例
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: 模型加载或预热失败
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

    /// 检测图片中的人脸，返回包含边界框、关键点和置信度的检测结果列表
    ///
    /// # 参数
    /// - `image_bytes`: 原始图片的字节数据
    ///
    /// # 返回
    /// 返回检测到的人脸列表
    ///
    /// # 错误
    /// - `FaceEngineError::ImageError`: 图片解码失败
    /// - `FaceEngineError::OrtError`: 推理执行失败
    pub fn detect_faces(&self, image_bytes: &[u8]) -> Result<Vec<FaceDetection>, FaceEngineError> {
        self.detector.detect(image_bytes)
    }

    /// 从已对齐的人脸图像中提取 512 维特征向量
    ///
    /// # 参数
    /// - `aligned_face`: 已对齐的 112x112 人脸图像
    ///
    /// # 返回
    /// 返回 L2 归一化后的 512 维特征向量
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: 推理执行失败
    /// - `FaceEngineError::InvalidOutput`: 模型输出维度不匹配
    pub fn extract_embedding(&self, aligned_face: &DynamicImage) -> Result<[f32; 512], FaceEngineError> {
        self.recognizer.extract(aligned_face)
    }

    /// 一站式检测图片中所有人脸并提取特征向量
    ///
    /// # 参数
    /// - `image_bytes`: 原始图片的字节数据
    ///
    /// # 返回
    /// 返回检测结果与对应特征向量的配对列表
    ///
    /// # 错误
    /// - `FaceEngineError::ImageError`: 图片解码失败
    /// - `FaceEngineError::OrtError`: 推理执行失败
    /// - `FaceEngineError::AlignmentFailed`: 人脸对齐失败
    /// - `FaceEngineError::InvalidOutput`: 模型输出维度不匹配
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
