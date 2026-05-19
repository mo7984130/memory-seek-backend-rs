use crate::FaceEngineError;
use image::DynamicImage;
use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider};
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Value;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

pub struct OrtSession {
    session: Mutex<ort::session::Session>,
    input_size: u32,
}

impl OrtSession {
    /// 从 ONNX 模型文件创建推理会话，支持 CUDA 和 CPU 执行提供程序
    ///
    /// # 参数
    /// - `model_path`: ONNX 模型文件路径
    /// - `input_size`: 模型输入图像的边长（正方形）
    /// - `model_name`: 模型名称，用于日志输出
    ///
    /// # 返回
    /// 返回初始化完成的 `OrtSession` 实例
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: 路径无效、模型文件读取失败或 ONNX Runtime 初始化失败
    pub fn new(model_path: &str, input_size: u32, model_name: &str) -> Result<Self, FaceEngineError> {
        let full_path = Path::new(model_path)
            .canonicalize()
            .map_err(|e| FaceEngineError::OrtError(format!("路径无效: {}", e)))?;

        info!("加载 {} 模型，路径: {:?}", model_name, full_path);

        let model_bytes = fs::read(&full_path)
            .map_err(|e| FaceEngineError::OrtError(format!("读取模型失败: {}", e)))?;
        info!("模型文件大小: {} bytes", model_bytes.len());

        let session = ort::session::Session::builder()
            .map_err(|e| FaceEngineError::OrtError(format!("创建 builder 失败: {}", e)))?
            .with_optimization_level(GraphOptimizationLevel::Level1)
            .map_err(|e| FaceEngineError::OrtError(format!("设置优化级别失败: {}", e)))?
            .with_intra_threads(4)
            .map_err(|e| FaceEngineError::OrtError(format!("设置线程数失败: {}", e)))?
            .with_execution_providers([
                CUDAExecutionProvider::default().build(),
                CPUExecutionProvider::default().build(),
            ])
            .map_err(|e| FaceEngineError::OrtError(format!("设置执行提供程序失败: {}", e)))?
            .commit_from_memory(&model_bytes)
            .map_err(|e| FaceEngineError::OrtError(format!("加载模型失败: {}", e)))?;

        info!("{} model loaded successfully", model_name);
        Ok(Self {
            session: Mutex::new(session),
            input_size,
        })
    }

    /// 使用全零输入预热模型，确保首次推理时延迟稳定
    ///
    /// # 参数
    /// - `model_name`: 模型名称，用于日志输出
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: ONNX Runtime 推理执行失败
    pub fn warmup(&self, model_name: &str) -> Result<(), FaceEngineError> {
        info!("Warming up {}...", model_name);
        let (shape, input_data) = self.preprocess_tensor();

        let input_value = Value::from_array((shape, input_data))
            .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

        let inputs = ort::inputs![input_value];
        let mut session = self.session.lock()
            .map_err(|_| FaceEngineError::OrtError("Failed to lock session".to_string()))?;
        session
            .run(inputs)
            .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

        info!("{} warmup complete", model_name);
        Ok(())
    }

    /// 对图像进行预处理后执行推理，并通过回调函数处理输出
    ///
    /// # 参数
    /// - `img`: 输入的动态图像
    /// - `f`: 处理推理输出的回调函数
    ///
    /// # 返回
    /// 返回回调函数处理后的结果
    ///
    /// # 错误
    /// - `FaceEngineError::OrtError`: ONNX Runtime 推理执行失败
    pub fn run_inference<F, R>(&self, img: &DynamicImage, f: F) -> Result<R, FaceEngineError>
    where
        F: FnOnce(&ort::session::SessionOutputs) -> Result<R, FaceEngineError>,
    {
        let (shape, input_data) = Self::preprocess_image(img, self.input_size)?;

        let input_value = Value::from_array((shape, input_data))
            .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

        let inputs = ort::inputs![input_value];
        let mut session = self.session.lock()
            .map_err(|_| FaceEngineError::OrtError("Failed to lock session".to_string()))?;
        let outputs = session
            .run(inputs)
            .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

        f(&outputs)
    }

    // 生成全零的预热张量，形状为 [1, 3, input_size, input_size]
    fn preprocess_tensor(&self) -> (Vec<usize>, Vec<f32>) {
        let shape = vec![1usize, 3, self.input_size as usize, self.input_size as usize];
        let input_data: Vec<f32> = vec![0.0; shape.iter().product()];
        (shape, input_data)
    }

    // 将图像缩放到指定大小，转换为 NCHW 格式的归一化浮点张量
    fn preprocess_image(img: &DynamicImage, input_size: u32) -> Result<(Vec<usize>, Vec<f32>), FaceEngineError> {
        let resized = img.resize_exact(
            input_size,
            input_size,
            image::imageops::FilterType::Lanczos3,
        );

        let rgb_img = resized.to_rgb8();
        let (width, height) = rgb_img.dimensions();

        let shape = vec![1usize, 3, input_size as usize, input_size as usize];
        let mut input_data = vec![0.0f32; shape.iter().product()];

        for y in 0..height as usize {
            for x in 0..width as usize {
                let pixel = rgb_img.get_pixel(x as u32, y as u32);
                let [r, g, b] = pixel.0;

                let base = y * input_size as usize + x;
                input_data[base] = (b as f32 - 127.5) / 128.0;
                input_data[input_size as usize * input_size as usize + base] = (g as f32 - 127.5) / 128.0;
                input_data[2 * input_size as usize * input_size as usize + base] = (r as f32 - 127.5) / 128.0;
            }
        }

        Ok((shape, input_data))
    }
}
