//! 图片处理: 依配置在「对象存储服务端处理」与「本地处理」间切换.
//!
//! - OSS 后端: 复用阿里云 `x-oss-process=image/*` 服务端处理。
//! - 本地后端: 下载原图后用 `image` crate 缩放/裁剪并编码为 WebP。
//!
//! 两者产出语义一致(缩略图宽 300 / 预览图宽 1920 / 人脸裁剪宽 200, 均 WebP,
//! 仅缩不放), 由 [`crate::config::ImageProcessor`] 选择。

use bytes::Bytes;
use common_core::AppError;
use common_core::error::contextual::Result;
use common_core::error::contextual::ext::{IntoContextualExt, ResultContextualExt};
use image::{DynamicImage, ImageFormat, imageops::FilterType};
use oss::S3Client;
use std::io::Cursor;
use types_visual::{FaceBBox, ImageDimensions};

use crate::config::ResolvedImageProcessor;

/// 缩略图目标宽度
const THUMBNAIL_WIDTH: u32 = 300;
/// 预览图目标宽度
const PREVIEW_WIDTH: u32 = 1920;
/// 人脸裁剪目标宽度
const CROP_WIDTH: u32 = 200;

/// 图片处理意图(语义化, 与具体后端无关)
#[derive(Clone)]
pub enum ProcessOp {
    /// 缩略图: 缩放到宽 300
    Thumbnail,
    /// 预览图: 缩放到宽 1920
    Preview,
    /// 人脸裁剪: 按 bbox 裁剪后缩放到宽 200
    Crop {
        bbox: FaceBBox,
        source: ImageDimensions,
    },
}

impl ProcessOp {
    /// 转成阿里云 OSS 图片处理参数。
    fn oss_param(&self) -> String {
        match self {
            Self::Thumbnail => format!("image/resize,w_{THUMBNAIL_WIDTH}/format,webp"),
            Self::Preview => format!("image/resize,w_{PREVIEW_WIDTH}/format,webp"),
            Self::Crop { bbox, source } => {
                let (x, y, w, h) = bbox.to_pixel_rect(source.width, source.height);
                format!("image/crop,x_{x},y_{y},w_{w},h_{h}/resize,w_{CROP_WIDTH}/format,webp")
            }
        }
    }
}

/// 依后端处理图片, 返回处理产物字节(WebP)。
pub async fn process_image(
    s3: &S3Client,
    backend: ResolvedImageProcessor,
    file_id: &str,
    op: &ProcessOp,
) -> Result<Bytes> {
    match backend {
        ResolvedImageProcessor::Oss => s3
            .download_with_process(file_id, &op.oss_param())
            .await
            .into_contextual(),
        ResolvedImageProcessor::Local => {
            let original = s3.download(file_id).await.into_contextual()?;
            let op = op.clone();
            // 解码/缩放/编码为 CPU 密集, 放到阻塞线程池避免占用异步执行器
            tokio::task::spawn_blocking(move || local_process(&original, &op))
                .await
                .into_contextual()?
        }
    }
}

/// 本地处理: 解码 -> 缩放/裁剪 -> 编码 WebP。
fn local_process(bytes: &[u8], op: &ProcessOp) -> Result<Bytes> {
    let image = image::load_from_memory(bytes).context_err(
        "local_image_decode_err",
        "本地图片解码失败",
        AppError::bad_request("图片解码失败"),
    )?;

    let processed = match op {
        ProcessOp::Thumbnail => fit_width(image, THUMBNAIL_WIDTH),
        ProcessOp::Preview => fit_width(image, PREVIEW_WIDTH),
        ProcessOp::Crop { bbox, source } => {
            let (x, y, w, h) = bbox.to_pixel_rect(source.width, source.height);
            // to_pixel_rect 返回 i32 且可能越界, 转 u32 并钳制到图像范围内
            let x = x.max(0) as u32;
            let y = y.max(0) as u32;
            let w = (w.max(0) as u32).clamp(1, image.width().saturating_sub(x).max(1));
            let h = (h.max(0) as u32).clamp(1, image.height().saturating_sub(y).max(1));
            fit_width(image.crop_imm(x, y, w, h), CROP_WIDTH)
        }
    };

    let mut buffer = Vec::new();
    processed
        .write_to(&mut Cursor::new(&mut buffer), ImageFormat::WebP)
        .context_err(
            "local_image_encode_err",
            "本地图片编码失败",
            AppError::InternalServerError,
        )?;
    Ok(Bytes::from(buffer))
}

/// 按目标宽度等比缩放(仅缩小, 不放大)。
fn fit_width(image: DynamicImage, max_width: u32) -> DynamicImage {
    if image.width() <= max_width || image.width() == 0 {
        return image;
    }
    let ratio = image.height() as f64 / image.width() as f64;
    let target_height = ((max_width as f64 * ratio).round() as u32).max(1);
    image.resize(max_width, target_height, FilterType::Lanczos3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    /// 生成纯色图片并按 PNG 编码
    fn sample_png(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_pixel(width, height, Rgb([10, 20, 30]));
        let mut buffer = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
            .unwrap();
        buffer
    }

    fn decode(bytes: &[u8]) -> DynamicImage {
        image::load_from_memory(bytes).unwrap()
    }

    #[test]
    fn thumbnail_downscales_to_target_width() {
        let output = local_process(&sample_png(800, 600), &ProcessOp::Thumbnail).unwrap();
        assert_eq!(decode(&output).width(), 300);
        // WebP RIFF 头
        assert_eq!(&output[..4], b"RIFF");
        assert_eq!(&output[8..12], b"WEBP");
    }

    #[test]
    fn preview_downscales_to_target_width() {
        let output = local_process(&sample_png(4000, 3000), &ProcessOp::Preview).unwrap();
        assert_eq!(decode(&output).width(), 1920);
    }

    #[test]
    fn small_image_is_not_upscaled() {
        let output = local_process(&sample_png(100, 80), &ProcessOp::Thumbnail).unwrap();
        assert_eq!(decode(&output).width(), 100);
    }

    #[test]
    fn crop_matches_bbox_and_target_width() {
        let op = ProcessOp::Crop {
            bbox: FaceBBox {
                x1: 0.0,
                y1: 0.0,
                x2: 0.5,
                y2: 0.5,
            },
            source: ImageDimensions {
                width: 800,
                height: 800,
            },
        };
        let output = local_process(&sample_png(800, 800), &op).unwrap();
        // 裁剪区 400x400 再缩放到宽 200
        assert_eq!(decode(&output).width(), 200);
    }
}
