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
use common_core::error::contextual::ext::{IntoContextualExt, ResultExt};
use futures::{Stream, stream};
use image::{DynamicImage, ImageDecoder as _, ImageFormat, imageops::FilterType};
use oss::{OssError, S3Client};
use std::io::Cursor;
use std::pin::Pin;
use types_visual::ImageTokenType;

use crate::config::ResolvedImageProcessor;

/// 缩略图目标宽度
const THUMBNAIL_WIDTH: u32 = 300;
/// 预览图目标宽度
const PREVIEW_WIDTH: u32 = 1920;
/// 人脸裁剪目标宽度
const CROP_WIDTH: u32 = 200;

/// 有损 WebP 质量(1-100); 缩略图更小, 预览保留更多细节
const THUMBNAIL_WEBP_QUALITY: u8 = 75;
const PREVIEW_WEBP_QUALITY: u8 = 80;
const CROP_WEBP_QUALITY: u8 = 80;

/// 转成阿里云 OSS 图片处理参数(有损 WebP, 缩略图/预览自动摆正)
fn oss_param(op: &ImageTokenType) -> String {
    match op {
        ImageTokenType::Thumbnail => format!(
            "image/resize,w_{THUMBNAIL_WIDTH}/auto-orient,1/format,webp/quality,Q_{THUMBNAIL_WEBP_QUALITY}"
        ),
        ImageTokenType::Preview => format!(
            "image/resize,w_{PREVIEW_WIDTH}/auto-orient,1/format,webp/quality,Q_{PREVIEW_WEBP_QUALITY}"
        ),
        ImageTokenType::Crop {
            bbox,
            source_dimensions,
        } => {
            let (x, y, w, h) =
                bbox.to_pixel_rect(source_dimensions.width, source_dimensions.height);
            format!(
                "image/crop,x_{x},y_{y},w_{w},h_{h}/resize,w_{CROP_WIDTH}/format,webp/quality,Q_{CROP_WEBP_QUALITY}"
            )
        }
        ImageTokenType::Original => unreachable!("Original 不参与图片处理"),
    }
}

/// 处理方式标识(含后端与参数), 与 `file_id` 共同构成产物对象 key。
/// `v2` 对应有损 WebP + 自动摆正; 升级参数时必须更新版本号。
pub(crate) fn op_key(op: &ImageTokenType, backend: ResolvedImageProcessor) -> String {
    let backend = match backend {
        ResolvedImageProcessor::Oss => "oss",
        ResolvedImageProcessor::Local => "local",
    };
    match op {
        ImageTokenType::Thumbnail => format!("{backend}:v2:thumb"),
        ImageTokenType::Preview => format!("{backend}:v2:preview"),
        ImageTokenType::Crop {
            bbox,
            source_dimensions,
        } => {
            let (x, y, w, h) =
                bbox.to_pixel_rect(source_dimensions.width, source_dimensions.height);
            format!("{backend}:v2:crop:{x}_{y}_{w}_{h}")
        }
        ImageTokenType::Original => unreachable!("Original 不参与图片处理"),
    }
}

/// 处理产物在对象存储中的 key
pub(crate) fn processed_object_key(file_id: &str, op_key: &str) -> String {
    format!("processed/{file_id}/{op_key}")
}

/// 每种派生图的有损 WebP 质量
fn webp_quality(op: &ImageTokenType) -> u8 {
    match op {
        ImageTokenType::Thumbnail => THUMBNAIL_WEBP_QUALITY,
        ImageTokenType::Preview => PREVIEW_WEBP_QUALITY,
        ImageTokenType::Crop { .. } => CROP_WEBP_QUALITY,
        ImageTokenType::Original => unreachable!("Original 不参与图片处理"),
    }
}

/// 图片处理产物: 字节与按魔数识别的实际格式
pub struct ProcessedImage {
    /// 处理产物字节
    pub bytes: Bytes,
    /// 产物实际格式(按字节魔数识别)
    pub format: ImageFormat,
}

/// 依后端处理图片并返回产物
pub async fn process_image(
    s3: &S3Client,
    backend: ResolvedImageProcessor,
    file_id: &str,
    op: &ImageTokenType,
) -> Result<ProcessedImage> {
    let bytes = render(s3, backend, file_id, op).await?;
    let format = image::guess_format(&bytes).context_error(
        "processed_image_format_err",
        "图片处理产物格式识别失败",
        AppError::InternalServerError,
    )?;
    Ok(ProcessedImage { bytes, format })
}

/// 把已处理好的字节包装成单块流
pub(crate) fn single_chunk_stream(
    bytes: Bytes,
) -> Pin<Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>> {
    Box::pin(stream::iter([Ok::<_, OssError>(bytes)]))
}

/// 按字节魔数返回图片 MIME; 无法识别时回退为二进制
pub(crate) fn sniff_image_content_type(bytes: &[u8]) -> &'static str {
    image::guess_format(bytes)
        .map(|format| format.to_mime_type())
        .unwrap_or("application/octet-stream")
}

/// 预览图流式产物
pub struct ProcessedStream {
    /// 字节流
    pub stream: Pin<Box<dyn Stream<Item = std::result::Result<Bytes, OssError>> + Send>>,
    /// 响应 MIME
    pub content_type: String,
    /// 已知的响应体字节数
    pub content_length: Option<u64>,
}

/// 以流式方式返回预览图:
/// OSS 后端直接流式拉取服务端处理结果; 本地后端处理后包装为单块流。
pub async fn stream_processed_image(
    s3: &S3Client,
    backend: ResolvedImageProcessor,
    file_id: &str,
    op: &ImageTokenType,
) -> Result<ProcessedStream> {
    match backend {
        ResolvedImageProcessor::Oss => {
            let stream = s3
                .download_stream_with_process(file_id, &oss_param(op))
                .await
                .into_contextual()?;
            Ok(ProcessedStream {
                stream: Box::pin(stream),
                content_type: "image/webp".to_string(),
                content_length: None,
            })
        }
        ResolvedImageProcessor::Local => {
            let processed = process_image(s3, backend, file_id, op).await?;
            let content_length = processed.bytes.len() as u64;
            Ok(ProcessedStream {
                stream: single_chunk_stream(processed.bytes),
                content_type: processed.format.to_mime_type().to_string(),
                content_length: Some(content_length),
            })
        }
    }
}

/// 执行图片处理
async fn render(
    s3: &S3Client,
    backend: ResolvedImageProcessor,
    file_id: &str,
    op: &ImageTokenType,
) -> Result<Bytes> {
    match backend {
        ResolvedImageProcessor::Oss => s3
            .download_with_process(file_id, &oss_param(op))
            .await
            .into_contextual(),
        ResolvedImageProcessor::Local => {
            let original = s3.download(file_id).await.into_contextual()?;
            let op = *op;
            // 解码/缩放/编码为 CPU 密集, 放到阻塞线程池避免占用异步执行器
            tokio::task::spawn_blocking(move || local_process(&original, &op))
                .await
                .into_contextual()?
        }
    }
}

/// 本地处理图片: 解码后缩放或裁剪并编码为有损 WebP
fn local_process(bytes: &[u8], op: &ImageTokenType) -> Result<Bytes> {
    // 缩略图/预览按 EXIF 方向摆正; 裁剪沿用原始坐标, 不额外旋转
    let image = decode_for_process(bytes, !matches!(op, ImageTokenType::Crop { .. }))?;

    let processed = match op {
        ImageTokenType::Thumbnail => fit_width(image, THUMBNAIL_WIDTH),
        ImageTokenType::Preview => fit_width(image, PREVIEW_WIDTH),
        ImageTokenType::Crop {
            bbox,
            source_dimensions,
        } => {
            let (x, y, w, h) =
                bbox.to_pixel_rect(source_dimensions.width, source_dimensions.height);
            // to_pixel_rect 返回 i32 且可能越界, 转 u32 并钳制到图像范围内
            let x = x.max(0) as u32;
            let y = y.max(0) as u32;
            let w = (w.max(0) as u32).clamp(1, image.width().saturating_sub(x).max(1));
            let h = (h.max(0) as u32).clamp(1, image.height().saturating_sub(y).max(1));
            fit_width(image.crop_imm(x, y, w, h), CROP_WIDTH)
        }
        ImageTokenType::Original => unreachable!("Original 不参与图片处理"),
    };

    let quality = f32::from(webp_quality(op));
    let (width, height) = (processed.width(), processed.height());
    // 本地编码用静态链接的 libwebp; 客户端仍可用 image 的纯 Rust 解码
    let encoded = if processed.has_alpha() {
        let rgba = processed.to_rgba8();
        webp::Encoder::from_rgba(rgba.as_raw(), width, height).encode(quality)
    } else {
        let rgb = processed.to_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), width, height).encode(quality)
    };
    Ok(Bytes::from(encoded.to_vec()))
}

/// 解码图片; `orient` 为 true 时按 EXIF 方向摆正
fn decode_for_process(bytes: &[u8], orient: bool) -> Result<DynamicImage> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .context_error(
            "local_image_guess_err",
            "图片格式识别失败",
            AppError::bad_request("图片解码失败"),
        )?;
    let mut decoder = reader.into_decoder().context_error(
        "local_image_decoder_err",
        "图片解码器初始化失败",
        AppError::bad_request("图片解码失败"),
    )?;
    let orientation = if orient {
        decoder
            .orientation()
            .unwrap_or(image::metadata::Orientation::NoTransforms)
    } else {
        image::metadata::Orientation::NoTransforms
    };
    let mut image = DynamicImage::from_decoder(decoder).context_error(
        "local_image_decode_err",
        "本地图片解码失败",
        AppError::bad_request("图片解码失败"),
    )?;
    if orient {
        image.apply_orientation(orientation);
    }
    Ok(image)
}

/// 按目标宽度等比缩放图片, 仅缩小不放大
fn fit_width(image: DynamicImage, max_width: u32) -> DynamicImage {
    if image.width() <= max_width || image.width() == 0 {
        return image;
    }
    let ratio = image.height() as f64 / image.width() as f64;
    let target_height = ((max_width as f64 * ratio).round() as u32).max(1);
    image.resize(max_width, target_height, FilterType::Lanczos3)
}

/// 等比缩放图片到 `max_width×max_height` 框内, 仅缩小不放大
fn fit_within(image: DynamicImage, max_width: u32, max_height: u32) -> DynamicImage {
    if image.width() <= max_width && image.height() <= max_height {
        return image;
    }
    image.resize(max_width, max_height, FilterType::Lanczos3)
}

/// 供人脸检测取图: 按后端处理为限制在 `max_width×max_height` 内的位图(仅缩不放)
pub async fn fetch_for_detection(
    s3: &S3Client,
    backend: ResolvedImageProcessor,
    file_id: &str,
    max_width: u32,
    max_height: u32,
) -> Result<DynamicImage> {
    match backend {
        ResolvedImageProcessor::Oss => {
            let param = format!("image/resize,m_lfit,{max_width},{max_height}");
            let bytes = s3
                .download_with_process(file_id, &param)
                .await
                .into_contextual()?;
            decode(bytes).await
        }
        ResolvedImageProcessor::Local => {
            let bytes = s3.download(file_id).await.into_contextual()?;
            let image = decode(bytes).await?;
            tokio::task::spawn_blocking(move || fit_within(image, max_width, max_height))
                .await
                .into_contextual()
        }
    }
}

/// 在阻塞线程中解码图片字节
async fn decode(bytes: Bytes) -> Result<DynamicImage> {
    tokio::task::spawn_blocking(move || {
        image::load_from_memory(&bytes).context_error(
            "media_decode_err",
            "图片解码失败",
            AppError::bad_request("图片解码失败"),
        )
    })
    .await
    .into_contextual()?
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};
    use types_visual::{FaceBBox, ImageDimensions};

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
        let output = local_process(&sample_png(800, 600), &ImageTokenType::Thumbnail).unwrap();
        assert_eq!(decode(&output).width(), 300);
        // WebP RIFF 头
        assert_eq!(&output[..4], b"RIFF");
        assert_eq!(&output[8..12], b"WEBP");
    }

    #[test]
    fn oss_params_use_lossy_webp_and_auto_orientation() {
        let thumbnail = oss_param(&ImageTokenType::Thumbnail);
        assert!(thumbnail.contains("auto-orient,1"));
        assert!(thumbnail.contains("format,webp"));
        assert!(thumbnail.contains("quality,Q_75"));

        let preview = oss_param(&ImageTokenType::Preview);
        assert!(preview.contains("auto-orient,1"));
        assert!(preview.contains("quality,Q_80"));

        let crop = oss_param(&ImageTokenType::Crop {
            bbox: FaceBBox {
                x1: 0.0,
                y1: 0.0,
                x2: 0.5,
                y2: 0.5,
            },
            source_dimensions: ImageDimensions {
                width: 800,
                height: 800,
            },
        });
        assert!(crop.contains("quality,Q_80"));
    }

    #[test]
    fn preview_downscales_to_target_width() {
        let output = local_process(&sample_png(4000, 3000), &ImageTokenType::Preview).unwrap();
        assert_eq!(decode(&output).width(), 1920);
    }

    #[test]
    fn small_image_is_not_upscaled() {
        let output = local_process(&sample_png(100, 80), &ImageTokenType::Thumbnail).unwrap();
        assert_eq!(decode(&output).width(), 100);
    }

    #[test]
    fn processed_object_key_includes_file_id_and_op() {
        assert_eq!(
            processed_object_key("visuals/2026/10/26/a.png", "local:v2:thumb"),
            "processed/visuals/2026/10/26/a.png/local:v2:thumb"
        );
    }

    #[test]
    fn op_key_includes_backend_and_operation() {
        assert_eq!(
            op_key(&ImageTokenType::Thumbnail, ResolvedImageProcessor::Local),
            "local:v2:thumb"
        );
        assert_eq!(
            op_key(&ImageTokenType::Preview, ResolvedImageProcessor::Oss),
            "oss:v2:preview"
        );
    }

    #[test]
    fn fit_within_downscales_and_never_upscales() {
        let big = DynamicImage::ImageRgb8(RgbImage::from_pixel(4000, 2000, Rgb([1, 2, 3])));
        let fitted = fit_within(big, 1920, 1920);
        assert_eq!((fitted.width(), fitted.height()), (1920, 960));

        let small = DynamicImage::ImageRgb8(RgbImage::from_pixel(100, 80, Rgb([1, 2, 3])));
        let kept = fit_within(small, 1920, 1920);
        assert_eq!((kept.width(), kept.height()), (100, 80));
    }

    #[test]
    fn crop_matches_bbox_and_target_width() {
        let op = ImageTokenType::Crop {
            bbox: FaceBBox {
                x1: 0.0,
                y1: 0.0,
                x2: 0.5,
                y2: 0.5,
            },
            source_dimensions: ImageDimensions {
                width: 800,
                height: 800,
            },
        };
        let output = local_process(&sample_png(800, 800), &op).unwrap();
        // 裁剪区 400x400 再缩放到宽 200
        assert_eq!(decode(&output).width(), 200);
    }
}
