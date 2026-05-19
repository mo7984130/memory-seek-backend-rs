use crate::base::OrtSession;
use crate::FaceEngineError;
use image::DynamicImage;

const INPUT_SIZE: u32 = 112;
const EMBEDDING_DIM: usize = 512;

pub struct Recognizer {
    session: OrtSession,
}

impl Recognizer {
    /// 创建 ArcFace 人脸特征提取器，加载指定路径的 ONNX 模型
    ///
    /// # 参数
    /// - `model_path`: ArcFace 识别模型的 ONNX 文件路径
    ///
    /// # 返回
    /// 返回初始化完成的 `Recognizer` 实例
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: 模型加载失败
    pub fn new(model_path: &str) -> Result<Self, FaceEngineError> {
        let session = OrtSession::new(model_path, INPUT_SIZE, "ArcFace recognizer")?;
        Ok(Self { session })
    }

    /// 使用全零输入预热识别模型，确保首次推理时延迟稳定
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: ONNX Runtime 推理执行失败
    pub fn warmup(&self) -> Result<(), FaceEngineError> {
        self.session.warmup("ArcFace recognizer")
    }

    /// 从已对齐的人脸图像中提取 512 维特征向量并执行 L2 归一化
    ///
    /// # 参数
    /// - `aligned_face`: 已对齐的 112x112 人脸图像
    ///
    /// # 返回
    /// 返回 L2 归一化后的 512 维特征向量
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: ONNX Runtime 推理执行失败
    /// - `FaceEngineError::InvalidOutput`: 模型输出维度不等于 512
    pub fn extract(&self, aligned_face: &DynamicImage) -> Result<[f32; 512], FaceEngineError> {
        self.session.run_inference(aligned_face, |outputs| {
            Self::postprocess(outputs)
        })
    }

    // 从模型输出中提取 512 维特征向量并执行 L2 归一化
    fn postprocess(outputs: &ort::session::SessionOutputs) -> Result<[f32; 512], FaceEngineError> {
        let (_, output_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

        if output_data.len() != EMBEDDING_DIM {
            return Err(FaceEngineError::InvalidOutput(format!(
                "Expected embedding dimension {}, got {}",
                EMBEDDING_DIM,
                output_data.len()
            )));
        }

        let mut embedding = [0.0f32; 512];
        embedding.copy_from_slice(output_data);

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for e in embedding.iter_mut() {
                *e /= norm;
            }
        }

        Ok(embedding)
    }
}
