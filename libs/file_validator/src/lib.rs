use const_format::formatcp;
use nom_exif::{MediaParser, MediaSource, TrackInfo, TrackInfoTag};
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use thiserror::Error;

/// 图片/视频文件头魔数
const JPEG_MAGIC: [u8; 3] = [0xFF, 0xD8, 0xFF];
const PNG_MAGIC: [u8; 4] = [0x89, 0x50, 0x4E, 0x47];
/// ISOBMFF(MP4/MOV): 偏移 4 处为 `ftyp` box 标识
const ISOBMFF_TAG: [u8; 4] = *b"ftyp";
/// EBML(WebM/MKV) 魔数
const EBML_MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];
/// QuickTime 的 major brand
const QT_BRAND: [u8; 4] = *b"qt  ";

/// 魔数嗅探的媒体大类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
}

/// 由内容嗅探确定的规范格式定义
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContentFormat {
    ext: &'static str,
    mime_type: &'static str,
}

impl ContentFormat {
    const JPEG: Self = Self {
        ext: "jpg",
        mime_type: "image/jpeg",
    };
    const PNG: Self = Self {
        ext: "png",
        mime_type: "image/png",
    };
    const MP4: Self = Self {
        ext: "mp4",
        mime_type: "video/mp4",
    };
    const MOV: Self = Self {
        ext: "mov",
        mime_type: "video/quicktime",
    };
    const WEBM: Self = Self {
        ext: "webm",
        mime_type: "video/webm",
    };
    const MKV: Self = Self {
        ext: "mkv",
        mime_type: "video/x-matroska",
    };
}

/// 图片文件解析后的元数据
#[derive(Debug, Clone)]
pub struct ImageMetaData {
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub mime_type: String,
}

/// 视频文件解析后的元数据
#[derive(Debug, Clone)]
pub struct VideoMetaData {
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub size: u64,
    pub mime_type: String,
}

/// 图片与视频统一解析后的元数据
#[derive(Debug, Clone)]
pub struct FileMetaData {
    pub format: String,
    pub width: u32,
    pub height: u32,
    /// 视频时长(毫秒);图片为 `None`
    pub duration_ms: Option<u64>,
    pub size: u64,
    pub mime_type: String,
}

impl FileMetaData {
    /// 是否为视频文件
    pub fn is_video(&self) -> bool {
        self.duration_ms.is_some()
    }
}

impl From<ImageMetaData> for FileMetaData {
    fn from(meta: ImageMetaData) -> Self {
        Self {
            format: meta.format,
            width: meta.width,
            height: meta.height,
            duration_ms: None,
            size: meta.size,
            mime_type: meta.mime_type,
        }
    }
}

impl From<VideoMetaData> for FileMetaData {
    fn from(meta: VideoMetaData) -> Self {
        Self {
            format: meta.format,
            width: meta.width,
            height: meta.height,
            duration_ms: Some(meta.duration_ms),
            size: meta.size,
            mime_type: meta.mime_type,
        }
    }
}

/// 文件校验错误类型
#[derive(Error, Debug)]
pub enum FileValidationError {
    #[error("文件不能为空")]
    EmptyFile,
    #[error("{}", FileValidator::SIZE_ERROR_MSG)]
    TooLarge,
    #[error("{}", FileValidator::VIDEO_SIZE_ERROR_MSG)]
    VideoTooLarge,
    #[error("不支持的文件类型")]
    UnsupportedFileType,
    #[error("图片解析失败: {0}")]
    ParseError(String),
    #[error("视频解析失败: {0}")]
    VideoParseError(String),
    #[error("读取文件失败: {0}")]
    ReadError(String),
}

/// 媒体文件校验器, 全部基于文件头魔数嗅探, 不信任客户端声明的文件名/扩展名。
pub struct FileValidator;

impl FileValidator {
    pub const ALLOW_IMAGE_MAX_SIZE: u64 = 20 * 1024 * 1024;
    pub const ALLOW_VIDEO_MAX_SIZE: u64 = 512 * 1024 * 1024;
    const SIZE_ERROR_MSG: &'static str = formatcp!(
        "上传文件大小不能超过 {}MB",
        FileValidator::ALLOW_IMAGE_MAX_SIZE / 1024 / 1024
    );
    const VIDEO_SIZE_ERROR_MSG: &'static str = formatcp!(
        "上传视频文件大小不能超过 {}MB",
        FileValidator::ALLOW_VIDEO_MAX_SIZE / 1024 / 1024
    );

    /// 按文件头魔数嗅探媒体大类, 不依赖文件名。
    ///
    /// 头部不足魔数长度时返回 `None`。JPEG/PNG → [`MediaKind::Image`],
    /// ISOBMFF(`ftyp`)/EBML → [`MediaKind::Video`]。
    pub fn sniff_media(head: &[u8]) -> Option<MediaKind> {
        if head.len() >= JPEG_MAGIC.len() && head[..JPEG_MAGIC.len()] == JPEG_MAGIC {
            return Some(MediaKind::Image);
        }
        if head.len() >= PNG_MAGIC.len() && head[..PNG_MAGIC.len()] == PNG_MAGIC {
            return Some(MediaKind::Image);
        }
        if head.len() >= 8 && head[4..8] == ISOBMFF_TAG {
            return Some(MediaKind::Video);
        }
        if head.len() >= EBML_MAGIC.len() && head[..EBML_MAGIC.len()] == EBML_MAGIC {
            return Some(MediaKind::Video);
        }
        None
    }

    /// 校验媒体文件, 按魔数嗅探分发到图片/视频校验, 不依赖文件名。
    pub fn validate_visual(file_path: &Path) -> Result<FileMetaData, FileValidationError> {
        match Self::sniff_file(file_path)? {
            MediaKind::Image => Self::validate_image(file_path).map(Into::into),
            MediaKind::Video => Self::validate_video(file_path).map(Into::into),
        }
    }

    /// 校验图片文件(JPEG/PNG), 校验内容包括: 非空、20MB 上限、魔数匹配、尺寸解析。
    pub fn validate_image(file_path: &Path) -> Result<ImageMetaData, FileValidationError> {
        let mut file = Self::open_file(file_path)?;
        let size = file
            .metadata()
            .map_err(|e| FileValidationError::ReadError(e.to_string()))?
            .len();
        if size == 0 {
            return Err(FileValidationError::EmptyFile);
        }
        if size > Self::ALLOW_IMAGE_MAX_SIZE {
            return Err(FileValidationError::TooLarge);
        }

        let head = Self::read_head(&mut file, 8)?;
        let format =
            Self::detect_image_format(&head).ok_or(FileValidationError::UnsupportedFileType)?;
        let (width, height) = Self::extract_image_metadata(file_path)?;

        Ok(ImageMetaData {
            format: format.ext.to_string(),
            width,
            height,
            size,
            mime_type: format.mime_type.to_string(),
        })
    }

    /// 校验内存字节形式的图片(供小文件场景复用, 如头像上传), 语义与 [`Self::validate_image`] 一致。
    pub fn validate_image_mem(file_data: &[u8]) -> Result<ImageMetaData, FileValidationError> {
        if file_data.is_empty() {
            return Err(FileValidationError::EmptyFile);
        }
        if file_data.len() as u64 > Self::ALLOW_IMAGE_MAX_SIZE {
            return Err(FileValidationError::TooLarge);
        }

        let format =
            Self::detect_image_format(file_data).ok_or(FileValidationError::UnsupportedFileType)?;
        let (width, height) = {
            let cursor = Cursor::new(file_data);
            let reader = image::ImageReader::new(cursor)
                .with_guessed_format()
                .map_err(|e| FileValidationError::ParseError(e.to_string()))?;
            let dimensions = reader
                .into_dimensions()
                .map_err(|e| FileValidationError::ParseError(e.to_string()))?;
            (dimensions.0, dimensions.1)
        };

        Ok(ImageMetaData {
            format: format.ext.to_string(),
            width,
            height,
            size: file_data.len() as u64,
            mime_type: format.mime_type.to_string(),
        })
    }

    /// 校验视频文件(MP4/MOV/WebM/MKV), 校验内容包括: 非空、512MB 上限、魔数匹配、轨道元数据解析。
    pub fn validate_video(file_path: &Path) -> Result<VideoMetaData, FileValidationError> {
        let mut file = Self::open_file(file_path)?;
        let size = file
            .metadata()
            .map_err(|e| FileValidationError::ReadError(e.to_string()))?
            .len();
        if size == 0 {
            return Err(FileValidationError::EmptyFile);
        }
        if size > Self::ALLOW_VIDEO_MAX_SIZE {
            return Err(FileValidationError::VideoTooLarge);
        }

        // EBML 的 DocType(webm/matroska)在头部, 读足 32 字节以完成细分
        let head = Self::read_head(&mut file, 32)?;
        let format =
            Self::detect_video_format(&head).ok_or(FileValidationError::UnsupportedFileType)?;

        let track_info = Self::extract_video_metadata(file_path)?;

        let width = track_info
            .get(TrackInfoTag::Width)
            .and_then(|value| value.as_u32())
            .filter(|&width| width > 0)
            .ok_or_else(|| FileValidationError::VideoParseError("无法解析视频宽度".to_string()))?;
        let height = track_info
            .get(TrackInfoTag::Height)
            .and_then(|value| value.as_u32())
            .filter(|&height| height > 0)
            .ok_or_else(|| FileValidationError::VideoParseError("无法解析视频高度".to_string()))?;
        let duration_ms = track_info
            .get(TrackInfoTag::DurationMs)
            .and_then(|value| value.as_u64())
            .filter(|&duration_ms| duration_ms > 0)
            .ok_or_else(|| FileValidationError::VideoParseError("无法解析视频时长".to_string()))?;

        Ok(VideoMetaData {
            format: format.ext.to_string(),
            width,
            height,
            duration_ms,
            size,
            mime_type: format.mime_type.to_string(),
        })
    }

    /// 根据文件名扩展名返回受支持图片的规范 MIME 类型(仅用于下载响应推断)。
    pub fn image_content_type(file_name: &str) -> Option<&'static str> {
        let ext = Self::file_ext(file_name);
        match ext.as_str() {
            "jpg" | "jpeg" => Some(ContentFormat::JPEG.mime_type),
            "png" => Some(ContentFormat::PNG.mime_type),
            _ => None,
        }
    }

    /// 根据文件名扩展名返回受支持视频的规范 MIME 类型(仅用于下载响应推断)。
    pub fn video_content_type(file_name: &str) -> Option<&'static str> {
        let ext = Self::file_ext(file_name);
        match ext.as_str() {
            "mp4" => Some(ContentFormat::MP4.mime_type),
            "mov" => Some(ContentFormat::MOV.mime_type),
            "webm" => Some(ContentFormat::WEBM.mime_type),
            "mkv" => Some(ContentFormat::MKV.mime_type),
            _ => None,
        }
    }

    // 读文件头并嗅探媒体大类
    fn sniff_file(file_path: &Path) -> Result<MediaKind, FileValidationError> {
        let mut file = Self::open_file(file_path)?;
        let head = Self::read_head(&mut file, 8)?;
        Self::sniff_media(&head).ok_or(FileValidationError::UnsupportedFileType)
    }

    // 按魔数识别图片格式(JPEG/PNG)
    fn detect_image_format(head: &[u8]) -> Option<ContentFormat> {
        if head.len() >= JPEG_MAGIC.len() && head[..JPEG_MAGIC.len()] == JPEG_MAGIC {
            return Some(ContentFormat::JPEG);
        }
        if head.len() >= PNG_MAGIC.len() && head[..PNG_MAGIC.len()] == PNG_MAGIC {
            return Some(ContentFormat::PNG);
        }
        None
    }

    // 按魔数识别视频格式: ISOBMFF 读 major_brand 区分 MP4/MOV; EBML 读 DocType 区分 WebM/MKV
    fn detect_video_format(head: &[u8]) -> Option<ContentFormat> {
        if head.len() >= 12 && head[4..8] == ISOBMFF_TAG {
            // major_brand 位于 ftyp box 偏移 8
            return if head[8..12] == QT_BRAND {
                Some(ContentFormat::MOV)
            } else {
                Some(ContentFormat::MP4)
            };
        }
        if head.len() >= EBML_MAGIC.len() && head[..EBML_MAGIC.len()] == EBML_MAGIC {
            if Self::contains_ascii(head, b"webm") {
                return Some(ContentFormat::WEBM);
            }
            if Self::contains_ascii(head, b"matroska") {
                return Some(ContentFormat::MKV);
            }
            return None;
        }
        None
    }

    // 从文件名提取小写扩展名(忽略 ".gitignore" 等纯点文件), 空串表示无扩展名
    fn file_ext(file_name: &str) -> String {
        file_name
            .rsplit_once('.')
            .filter(|(base, _)| !base.is_empty())
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default()
    }

    // 在切片中查找 ASCII 子串
    fn contains_ascii(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    // 打开待校验文件, 打开失败统一映射为 ReadError
    fn open_file(file_path: &Path) -> Result<File, FileValidationError> {
        File::open(file_path).map_err(|e| FileValidationError::ReadError(e.to_string()))
    }

    // 读取文件头部指定字节数(不足则返回实际字节数)
    fn read_head(file: &mut File, n: usize) -> Result<Vec<u8>, FileValidationError> {
        let mut buf = vec![0u8; n];
        let mut read = 0;
        while read < n {
            let n_read = file
                .read(&mut buf[read..])
                .map_err(|e| FileValidationError::ReadError(e.to_string()))?;
            if n_read == 0 {
                break;
            }
            read += n_read;
        }
        buf.truncate(read);
        Ok(buf)
    }

    // 使用 nom-exif 解析视频轨道元数据，提取宽高、时长等信息
    fn extract_video_metadata(file_path: &Path) -> Result<TrackInfo, FileValidationError> {
        let mut parser = MediaParser::new();
        let source = MediaSource::open(file_path)
            .map_err(|e| FileValidationError::VideoParseError(e.to_string()))?;
        parser
            .parse_track(source)
            .map_err(|e| FileValidationError::VideoParseError(e.to_string()))
    }

    // 使用 image 库解析图片文件，提取宽高尺寸
    fn extract_image_metadata(file_path: &Path) -> Result<(u32, u32), FileValidationError> {
        let reader = image::ImageReader::open(file_path)
            .map_err(|e| FileValidationError::ParseError(e.to_string()))?
            .with_guessed_format()
            .map_err(|e| FileValidationError::ParseError(e.to_string()))?;

        let dimensions = reader
            .into_dimensions()
            .map_err(|e| FileValidationError::ParseError(e.to_string()))?;

        Ok((dimensions.0, dimensions.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 生成唯一临时文件路径(测试产物留在系统临时目录, 由系统清理)
    fn unique_temp_path() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "file_validator_test_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// 根据十六进制文件头和指定总大小创建模拟文件(稀疏写入, 不占用真实磁盘空间)
    fn create_mock_file(header_hex: &str, total_size: usize) -> PathBuf {
        let path = unique_temp_path();
        let mut file = File::create(&path).unwrap();
        for i in (0..header_hex.len()).step_by(2) {
            let byte = u8::from_str_radix(&header_hex[i..i + 2], 16).unwrap();
            file.write_all(&[byte]).unwrap();
        }
        if total_size > header_hex.len() / 2 {
            // 稀疏填充: seek 到末尾写一个字节, 文件长度即为 total_size
            file.seek(SeekFrom::Start(total_size as u64 - 1)).unwrap();
            file.write_all(&[0]).unwrap();
        }
        path
    }

    /// 将字节内容写入临时文件并返回路径
    fn write_bytes(data: &[u8]) -> PathBuf {
        let path = unique_temp_path();
        std::fs::write(&path, data).unwrap();
        path
    }

    #[test]
    fn sniff_media_detects_kinds_by_magic() {
        // 图片魔数
        assert_eq!(
            FileValidator::sniff_media(&[0xFF, 0xD8, 0xFF]),
            Some(MediaKind::Image)
        );
        assert_eq!(
            FileValidator::sniff_media(&[0x89, 0x50, 0x4E, 0x47]),
            Some(MediaKind::Image)
        );
        // 视频魔数: ftyp 位于偏移 4; EBML
        assert_eq!(
            FileValidator::sniff_media(b"\x00\x00\x00\x18ftypisom"),
            Some(MediaKind::Video)
        );
        assert_eq!(
            FileValidator::sniff_media(&[0x1A, 0x45, 0xDF, 0xA3, 0x00]),
            Some(MediaKind::Video)
        );
        // 非法/头部过短
        assert_eq!(FileValidator::sniff_media(&[0x50, 0x4B, 0x03, 0x04]), None);
        assert_eq!(FileValidator::sniff_media(&[0xFF, 0xD8]), None);
        assert_eq!(FileValidator::sniff_media(&[]), None);
    }

    #[test]
    fn detect_video_format_distinguishes_mov_and_mkv() {
        // QuickTime brand
        assert_eq!(
            FileValidator::detect_video_format(b"\x00\x00\x00\x18ftypqt  isom"),
            Some(ContentFormat::MOV)
        );
        // 其他 brand → MP4
        assert_eq!(
            FileValidator::detect_video_format(b"\x00\x00\x00\x18ftypisomisom"),
            Some(ContentFormat::MP4)
        );
        // EBML + DocType
        assert_eq!(
            FileValidator::detect_video_format(b"\x1A\x45\xDF\xA3\x93B\x86\x81webm\x00"),
            Some(ContentFormat::WEBM)
        );
        assert_eq!(
            FileValidator::detect_video_format(b"\x1A\x45\xDF\xA3\x93B\x86\x81matroska\x00"),
            Some(ContentFormat::MKV)
        );
        // EBML 但无有效 DocType
        assert_eq!(
            FileValidator::detect_video_format(&[0x1A, 0x45, 0xDF, 0xA3, 0x00]),
            None
        );
    }

    #[test]
    fn test_empty_file() {
        let path = unique_temp_path();
        File::create(&path).unwrap();

        let result = FileValidator::validate_image(&path);
        assert!(matches!(result, Err(FileValidationError::EmptyFile)));
    }

    #[test]
    fn test_file_too_large() {
        let path = create_mock_file("FFD8FF", (FileValidator::ALLOW_IMAGE_MAX_SIZE + 1) as usize);

        let result = FileValidator::validate_image(&path);
        assert!(matches!(result, Err(FileValidationError::TooLarge)));
    }

    #[test]
    fn test_mem_validation_empty_and_too_large() {
        assert!(matches!(
            FileValidator::validate_image_mem(&[]),
            Err(FileValidationError::EmptyFile)
        ));

        // 稀疏验证大小上限无需真实分配: 直接构造超长引用
        let big: Vec<u8> = vec![0xFF; (FileValidator::ALLOW_IMAGE_MAX_SIZE + 1) as usize];
        assert!(matches!(
            FileValidator::validate_image_mem(&big),
            Err(FileValidationError::TooLarge)
        ));
    }

    #[test]
    fn test_unsupported_content_rejected() {
        let path = write_bytes(&[0x50, 0x4B, 0x03, 0x04]);
        let result = FileValidator::validate_image(&path);
        assert!(matches!(
            result,
            Err(FileValidationError::UnsupportedFileType)
        ));
    }

    #[test]
    fn test_valid_image_parsing() {
        let tiny_png = hex::decode("89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415408D763F8FF7F0005FE02FE0DC444830000000049454E44AE426082").unwrap();
        let path = write_bytes(&tiny_png);

        let result = FileValidator::validate_image(&path);

        assert!(result.is_ok());
        let meta = result.unwrap();
        assert_eq!(meta.width, 1);
        assert_eq!(meta.height, 1);
        assert_eq!(meta.format, "png");
        assert_eq!(meta.mime_type, "image/png");
    }

    #[test]
    fn test_valid_image_mem_parsing() {
        let tiny_png = hex::decode("89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415408D763F8FF7F0005FE02FE0DC444830000000049454E44AE426082").unwrap();

        let meta = FileValidator::validate_image_mem(&tiny_png).unwrap();
        assert_eq!(meta.width, 1);
        assert_eq!(meta.height, 1);
        assert_eq!(meta.format, "png");
        assert_eq!(meta.mime_type, "image/png");
    }

    #[test]
    fn image_content_type_uses_the_supported_format_table() {
        assert_eq!(
            FileValidator::image_content_type("visuals/2026/08/17/visual.JPEG"),
            Some("image/jpeg")
        );
        assert_eq!(
            FileValidator::image_content_type("visuals/2026/08/17/visual.png"),
            Some("image/png")
        );
        assert_eq!(FileValidator::image_content_type("visual.webp"), None);
        assert_eq!(FileValidator::image_content_type("visual.gif"), None);
        assert_eq!(FileValidator::image_content_type("visual.bmp"), None);
        assert_eq!(FileValidator::image_content_type("visual."), None);
    }

    #[test]
    fn test_valid_jpeg_parsing() {
        let path = create_mock_file("FFD8FFE0", 100);

        let result = FileValidator::validate_image(&path);

        match result {
            Ok(_) => {}
            Err(FileValidationError::ParseError(_)) => {}
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    #[test]
    fn test_valid_png_header() {
        let path = create_mock_file("89504E47", 100);

        let result = FileValidator::validate_image(&path);

        match result {
            Ok(_) => {}
            Err(FileValidationError::ParseError(_)) => {}
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    #[test]
    fn test_case_insensitive_extension() {
        let path = write_bytes(&hex::decode("89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415408D763F8FF7F0005FE02FE0DC444830000000049454E44AE426082").unwrap());

        // 内容为 PNG, 校验只看魔数, 与命名无关
        let meta = FileValidator::validate_visual(&path).unwrap();
        assert_eq!(meta.format, "png");
    }

    // ---- 视频校验 ----

    // 测试样本取自 nom-exif 仓库 testdata（MIT 许可）：
    // - sample.mp4: Sony A7 XAVC 元数据样本（ftyp + moov，无媒体数据）
    // - sample.mov: QuickTime 样本（含 ftyp 品牌）
    const SAMPLE_MP4: &[u8] = include_bytes!("../tests/fixtures/sample.mp4");
    const SAMPLE_MOV: &[u8] = include_bytes!("../tests/fixtures/sample.mov");

    #[test]
    fn test_valid_mp4_parsing() {
        let path = write_bytes(SAMPLE_MP4);
        let result = FileValidator::validate_video(&path);

        let meta = result.expect("合法 MP4 应校验通过");
        assert!(meta.width > 0);
        assert!(meta.height > 0);
        assert!(meta.duration_ms > 0);
        assert_eq!(meta.format, "mp4");
        assert_eq!(meta.mime_type, "video/mp4");
        assert_eq!(meta.size, SAMPLE_MP4.len() as u64);
    }

    #[test]
    fn test_valid_mov_parsing() {
        let path = write_bytes(SAMPLE_MOV);
        let result = FileValidator::validate_video(&path);

        let meta = result.expect("合法 MOV 应校验通过");
        assert!(meta.width > 0);
        assert!(meta.height > 0);
        assert!(meta.duration_ms > 0);
        assert_eq!(meta.format, "mov");
        assert_eq!(meta.mime_type, "video/quicktime");
    }

    #[test]
    fn test_video_empty_file() {
        let path = unique_temp_path();
        File::create(&path).unwrap();
        let result = FileValidator::validate_video(&path);
        assert!(matches!(result, Err(FileValidationError::EmptyFile)));
    }

    #[test]
    fn test_video_unsupported_content() {
        let path = write_bytes(&[0xFF; 64]);
        let result = FileValidator::validate_video(&path);
        assert!(matches!(
            result,
            Err(FileValidationError::UnsupportedFileType)
        ));
    }

    #[test]
    fn test_video_header_too_short() {
        let path = write_bytes(&[0xFF; 8]);
        let result = FileValidator::validate_video(&path);
        assert!(matches!(
            result,
            Err(FileValidationError::UnsupportedFileType)
        ));
    }

    #[test]
    fn test_video_parse_failure() {
        // 文件头合法但缺少 moov 元数据，parse_track 应失败
        let path = create_mock_file("0000001866747970", 100);
        let result = FileValidator::validate_video(&path);
        assert!(matches!(
            result,
            Err(FileValidationError::VideoParseError(_))
        ));
    }

    #[test]
    fn test_ebml_without_doctype_rejected() {
        // EBML 魔数通过但缺少有效 DocType, 应报不支持而非解析错误
        let path = create_mock_file("1A45DFA3", 100);
        let result = FileValidator::validate_video(&path);
        assert!(matches!(
            result,
            Err(FileValidationError::UnsupportedFileType)
        ));
    }

    #[test]
    fn video_content_type_uses_the_supported_format_table() {
        assert_eq!(
            FileValidator::video_content_type("visuals/2026/09/16/visual.mp4"),
            Some("video/mp4")
        );
        assert_eq!(
            FileValidator::video_content_type("visuals/2026/09/16/visual.MOV"),
            Some("video/quicktime")
        );
        assert_eq!(
            FileValidator::video_content_type("visuals/2026/09/16/visual.webm"),
            Some("video/webm")
        );
        assert_eq!(
            FileValidator::video_content_type("visuals/2026/09/16/visual.mkv"),
            Some("video/x-matroska")
        );
        assert_eq!(FileValidator::video_content_type("visual.avi"), None);
        assert_eq!(FileValidator::video_content_type("visual."), None);
    }

    // ---- 统一入口 ----

    #[test]
    fn validate_visual_dispatches_video() {
        let path = write_bytes(SAMPLE_MP4);
        let meta = FileValidator::validate_visual(&path).expect("视频应校验通过");
        assert!(meta.is_video());
        assert_eq!(meta.duration_ms, Some(1440));
        assert_eq!(meta.mime_type, "video/mp4");
        assert_eq!(meta.format, "mp4");
    }

    #[test]
    fn validate_visual_dispatches_image() {
        let tiny_png = hex::decode("89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415408D763F8FF7F0005FE02FE0DC444830000000049454E44AE426082").unwrap();
        let path = write_bytes(&tiny_png);
        let meta = FileValidator::validate_visual(&path).expect("图片应校验通过");
        assert!(!meta.is_video());
        assert_eq!(meta.duration_ms, None);
        assert_eq!(meta.mime_type, "image/png");
        assert_eq!(meta.width, 1);
        assert_eq!(meta.height, 1);
    }

    #[test]
    fn validate_visual_rejects_unknown_content() {
        // 篡改 ftyp, 内容不再合法
        let mut data = SAMPLE_MOV.to_vec();
        data[4..8].copy_from_slice(b"XXXX");
        let path = write_bytes(&data);
        let result = FileValidator::validate_visual(&path);
        assert!(matches!(
            result,
            Err(FileValidationError::UnsupportedFileType)
        ));
    }

    #[test]
    fn file_not_found_maps_to_read_error() {
        let result =
            FileValidator::validate_image(Path::new("/nonexistent/definitely_missing.jpg"));
        assert!(matches!(result, Err(FileValidationError::ReadError(_))));
    }
}
