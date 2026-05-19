use const_format::formatcp;
use std::io::Cursor;
use thiserror::Error;

/// 图片文件解析后的元数据
#[derive(Debug, Clone)]
pub struct ImageMetaData {
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub name: String,
    pub mime_type: String,
}

/// 文件校验错误类型
#[derive(Error, Debug)]
pub enum FileValidationError {
    #[error("文件不能为空")]
    EmptyFile,
    #[error("文件名不能为空")]
    EmptyFileName,
    #[error("{}", FileValidator::SIZE_ERROR_MSG)]
    TooLarge,
    #[error("不支持的文件类型")]
    UnsupportedFileType,
    #[error("文件头不匹配")]
    InvalidHeader,
    #[error("图片解析失败: {0}")]
    ParseError(String),
}

/// 图片文件校验器，提供文件大小、类型、文件头等验证功能
pub struct FileValidator;

impl FileValidator {
    const ALLOW_IMAGE_MAX_SIZE: u64 = 20 * 1024 * 1024;
    const SIZE_ERROR_MSG: &'static str = formatcp!(
        "上传文件大小不能超过 {}MB",
        FileValidator::ALLOW_IMAGE_MAX_SIZE / 1024 / 1024
    );

    /// 校验图片文件的完整性，包括非空检查、大小限制、扩展名合法性、文件头匹配和图片尺寸解析
    ///
    /// # 参数
    /// - `file_data`: 图片文件的原始字节数据
    /// - `file_name`: 文件名，用于提取扩展名
    /// - `content_type`: MIME 类型字符串，写入返回的元数据
    ///
    /// # 返回
    /// 校验通过时返回 `ImageMetaData`，包含格式、宽高、大小等信息
    ///
    /// # 错误
    /// - `FileValidationError::EmptyFile`: 文件数据为空
    /// - `FileValidationError::TooLarge`: 文件超过 20MB 限制
    /// - `FileValidationError::EmptyFileName`: 文件名为空
    /// - `FileValidationError::UnsupportedFileType`: 扩展名不在支持列表中
    /// - `FileValidationError::InvalidHeader`: 文件头与预期格式不匹配
    /// - `FileValidationError::ParseError`: 图片尺寸解析失败
    pub fn validate_image(
        file_data: &[u8],
        file_name: &str,
        content_type: &str,
    ) -> Result<ImageMetaData, FileValidationError> {
        if file_data.is_empty() {
            return Err(FileValidationError::EmptyFile);
        }
        if file_data.len() as u64 > Self::ALLOW_IMAGE_MAX_SIZE {
            return Err(FileValidationError::TooLarge);
        }
        if file_name.is_empty() {
            return Err(FileValidationError::EmptyFileName);
        }

        let file_type = Self::extract_file_extension(file_name);
        if file_type.is_empty() {
            return Err(FileValidationError::UnsupportedFileType);
        }

        let expected_header = Self::get_expected_header(&file_type)?;
        Self::validate_file_header(file_data, expected_header)?;

        let (width, height) = Self::extract_image_metadata(file_data)?;

        Ok(ImageMetaData {
            format: file_type,
            width,
            height,
            size: file_data.len() as u64,
            name: file_name.to_string(),
            mime_type: content_type.to_string(),
        })
    }

    // 从文件名中提取小写扩展名，忽略 ".gitignore" 等纯点文件
    fn extract_file_extension(file_name: &str) -> String {
        file_name
            .rsplit_once('.')
            .filter(|(base, _)| !base.is_empty())
            .map(|(_, ext)| ext.to_lowercase())
            .unwrap_or_default()
    }

    // 根据文件扩展名返回预期的文件头十六进制字符串
    fn get_expected_header(file_type: &str) -> Result<&'static str, FileValidationError> {
        match file_type {
            "jpg" | "jpeg" => Ok("FFD8FF"),
            "png" => Ok("89504E47"),
            "gif" => Ok("47494638"),
            "bmp" => Ok("424D"),
            _ => Err(FileValidationError::UnsupportedFileType),
        }
    }

    // 校验文件头部字节是否与预期的十六进制签名匹配
    fn validate_file_header(
        file_data: &[u8],
        expected_header: &str,
    ) -> Result<(), FileValidationError> {
        let expected_bytes = expected_header.as_bytes();
        let header_byte_count = expected_bytes.len() / 2;

        if file_data.len() < header_byte_count {
            return Err(FileValidationError::InvalidHeader);
        }

        for (i, chunk) in expected_bytes.chunks(2).enumerate() {
            let hex_str =
                std::str::from_utf8(chunk).map_err(|_| FileValidationError::InvalidHeader)?;
            let expected = u8::from_str_radix(hex_str, 16)
                .map_err(|_| FileValidationError::InvalidHeader)?;
            if file_data[i] != expected {
                return Err(FileValidationError::InvalidHeader);
            }
        }
        Ok(())
    }

    // 使用 image 库解析图片数据，提取宽高尺寸
    fn extract_image_metadata(file_data: &[u8]) -> Result<(u32, u32), FileValidationError> {
        let cursor = Cursor::new(file_data);

        let reader = image::ImageReader::new(cursor)
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

    // 根据十六进制文件头和指定总大小创建模拟文件数据，用于测试文件校验逻辑
    fn create_mock_file(header_hex: &str, total_size: usize) -> Vec<u8> {
        let mut data = Vec::with_capacity(total_size);
        for i in (0..header_hex.len()).step_by(2) {
            data.push(u8::from_str_radix(&header_hex[i..i + 2], 16).unwrap());
        }
        data.resize(total_size, 0);
        data
    }

    #[test]
    fn test_empty_file() {
        let result = FileValidator::validate_image(&[], "test.jpg", "image/jpeg");
        assert!(matches!(result, Err(FileValidationError::EmptyFile)));
    }

    #[test]
    fn test_file_too_large() {
        // 用空切片 + 手动构造超大长度引用来避免实际分配 20MB 内存
        // 改为只测刚好超限的逻辑：分配一个 header + padding 刚好超限
        // 但 &[u8] 长度由实际数据决定，无法伪造，保留最小分配方式
        let size = (FileValidator::ALLOW_IMAGE_MAX_SIZE + 1) as usize;
        let big_data = vec![0xFFu8; size];
        let result = FileValidator::validate_image(&big_data, "big.jpg", "image/jpeg");
        assert!(matches!(result, Err(FileValidationError::TooLarge)));
    }

    #[test]
    fn test_unsupported_extension() {
        let data = create_mock_file("FFD8FF", 10);
        let result = FileValidator::validate_image(&data, "test.exe", "application/octet-stream");
        assert!(matches!(
            result,
            Err(FileValidationError::UnsupportedFileType)
        ));
    }

    #[test]
    fn test_invalid_header_mismatch() {
        let data = create_mock_file("FFD8FF", 10);
        let result = FileValidator::validate_image(&data, "test.png", "image/png");
        assert!(matches!(result, Err(FileValidationError::InvalidHeader)));
    }

    #[test]
    fn test_empty_file_name() {
        let data = create_mock_file("FFD8FF", 10);
        let result = FileValidator::validate_image(&data, "", "image/jpeg");
        assert!(matches!(result, Err(FileValidationError::EmptyFileName)));
    }

    #[test]
    fn test_file_too_small_for_header() {
        let small_data = vec![0xFFu8; 2];
        let result = FileValidator::validate_image(&small_data, "test.jpg", "image/jpeg");
        assert!(matches!(result, Err(FileValidationError::InvalidHeader)));
    }

    #[test]
    fn test_valid_image_parsing() {
        let tiny_png = hex::decode("89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415408D763F8FF7F0005FE02FE0DC444830000000049454E44AE426082").unwrap();

        let result = FileValidator::validate_image(&tiny_png, "pixel.png", "image/png");

        assert!(result.is_ok());
        let meta = result.unwrap();
        assert_eq!(meta.width, 1);
        assert_eq!(meta.height, 1);
        assert_eq!(meta.format, "png");
    }

    #[test]
    fn test_valid_jpeg_parsing() {
        let jpeg_data = create_mock_file("FFD8FFE0", 100);

        let result = FileValidator::validate_image(&jpeg_data, "test.jpg", "image/jpeg");

        match result {
            Ok(_) => {}
            Err(FileValidationError::ParseError(_)) => {}
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    #[test]
    fn test_valid_png_header() {
        let png_data = create_mock_file("89504E47", 100);

        let result = FileValidator::validate_image(&png_data, "test.png", "image/png");

        match result {
            Ok(_) => {}
            Err(FileValidationError::ParseError(_)) => {}
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    #[test]
    fn test_valid_gif_header() {
        let gif_data = create_mock_file("47494638", 100);

        let result = FileValidator::validate_image(&gif_data, "test.gif", "image/gif");

        match result {
            Ok(_) => {}
            Err(FileValidationError::ParseError(_)) => {}
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    #[test]
    fn test_valid_bmp_header() {
        let bmp_data = create_mock_file("424D", 100);

        let result = FileValidator::validate_image(&bmp_data, "test.bmp", "image/bmp");

        match result {
            Ok(_) => {}
            Err(FileValidationError::ParseError(_)) => {}
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    #[test]
    fn test_case_insensitive_extension() {
        let data = create_mock_file("FFD8FF", 100);

        let result = FileValidator::validate_image(&data, "test.JPG", "image/jpeg");

        match result {
            Ok(_) => {}
            Err(FileValidationError::ParseError(_)) => {}
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    #[test]
    fn test_dotfile_not_treated_as_extension() {
        // ".gitignore" 应该没有有效扩展名
        let data = create_mock_file("FFD8FF", 10);
        let result = FileValidator::validate_image(&data, ".gitignore", "application/octet-stream");
        assert!(matches!(
            result,
            Err(FileValidationError::UnsupportedFileType)
        ));
    }

    #[test]
    fn test_trailing_dot_no_extension() {
        let data = create_mock_file("FFD8FF", 10);
        let result = FileValidator::validate_image(&data, "test.", "image/jpeg");
        assert!(matches!(
            result,
            Err(FileValidationError::UnsupportedFileType)
        ));
    }
}
