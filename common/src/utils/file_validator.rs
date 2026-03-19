use const_format::formatcp;
use thiserror::Error;
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct ImageMetaData {
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub name: String,
    pub mime_type: String,
}

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
    ParseError(String), // 增加一个处理第三方库错误的变体
}

pub struct FileValidator;

impl FileValidator {
    const ALLOW_IMAGE_TYPE: &'static [(&'static str, &'static str)] = &[
        ("jpg", "FFD8FF"),
        ("jpeg", "FFD8FF"),
        ("png", "89504E47"),
        ("gif", "47494638"),
        ("bmp", "424D"),
    ];

    const ALLOW_IMAGE_MAX_SIZE: u64 = 20 * 1024 * 1024;

    const SIZE_ERROR_MSG: &'static str = formatcp!(
        "上传文件大小不能超过 {}MB",
        FileValidator::ALLOW_IMAGE_MAX_SIZE / 1024 / 1024
    );

    pub fn validate_image(
        file_data: &[u8],
        file_name: String,
        content_type: String,
    ) -> Result<ImageMetaData, FileValidationError> {
        if file_data.is_empty() { return Err(FileValidationError::EmptyFile); }
        if file_data.len() as u64 > Self::ALLOW_IMAGE_MAX_SIZE { return Err(FileValidationError::TooLarge); }
        if file_name.is_empty() { return Err(FileValidationError::EmptyFileName); }

        let file_type = Self::extract_file_extension(&file_name);
        if !Self::is_supported_format(&file_type) {
            return Err(FileValidationError::UnsupportedFileType);
        }

        Self::validate_file_header(file_data, Self::get_expected_header(&file_type)?)?;

        let (width, height, _) = Self::extract_image_metadata(file_data)?;

        Ok(ImageMetaData {
            format: file_type,
            width,
            height,
            size: file_data.len() as u64,
            name: file_name,
            mime_type: content_type,
        })
    }

    fn extract_file_extension(file_name: &str) -> String {
        file_name.rsplit('.').next().unwrap_or("").to_lowercase()
    }

    fn is_supported_format(file_type: &str) -> bool {
        Self::ALLOW_IMAGE_TYPE.iter().any(|(fmt, _)| *fmt == file_type)
    }

    fn get_expected_header(file_type: &str) -> Result<&'static str, FileValidationError> {
        Self::ALLOW_IMAGE_TYPE
            .iter()
            .find(|(fmt, _)| *fmt == file_type)
            .map(|(_, header)| *header)
            .ok_or(FileValidationError::InvalidHeader)
    }

    fn validate_file_header(file_data: &[u8], expected_header: &str) -> Result<(), FileValidationError> {
        if file_data.len() < 4 { return Err(FileValidationError::InvalidHeader); }

        let header_bytes = &file_data[..4];
        let file_header = Self::bytes_to_hex(header_bytes);

        // 修正：统一大小写对比
        if !file_header.to_uppercase().starts_with(expected_header) {
            return Err(FileValidationError::InvalidHeader);
        }
        Ok(())
    }

    fn extract_image_metadata(file_data: &[u8]) -> Result<(u32, u32, String), FileValidationError> {
        let cursor = Cursor::new(file_data);

        let reader = image::ImageReader::new(cursor)
            .with_guessed_format()
            .map_err(|e| FileValidationError::ParseError(e.to_string()))?;

        let format_str = reader.format()
            .map(|f| format!("{:?}", f))
            .unwrap_or_else(|| "unknown".to_string());

        let dimensions = reader.into_dimensions()
            .map_err(|e| FileValidationError::ParseError(e.to_string()))?;

        Ok((dimensions.0, dimensions.1, format_str))
    }

    fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02X}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 辅助工具：生成指定开头的字节数组
    fn create_mock_file(header_hex: &str, total_size: usize) -> Vec<u8> {
        let mut data = Vec::with_capacity(total_size);
        // 将 Hex 字符串转回字节
        for i in (0..header_hex.len()).step_by(2) {
            data.push(u8::from_str_radix(&header_hex[i..i+2], 16).unwrap());
        }
        data.resize(total_size, 0);
        data
    }

    /// 测试空文件的处理
    #[test]
    fn test_empty_file() {
        let result = FileValidator::validate_image(&[], "test.jpg".into(), "image/jpeg".into());
        assert!(matches!(result, Err(FileValidationError::EmptyFile)));
    }

    /// 测试超过最大文件大小限制（20MB）
    #[test]
    fn test_file_too_large() {
        // 模拟一个超过 20MB 的数据（实际上不用真的分配，只要长度够）
        let big_data = vec![0u8; (FileValidator::ALLOW_IMAGE_MAX_SIZE + 1) as usize];
        let result = FileValidator::validate_image(&big_data, "big.jpg".into(), "image/jpeg".into());
        assert!(matches!(result, Err(FileValidationError::TooLarge)));
    }

    /// 测试不支持的文件扩展名
    #[test]
    fn test_unsupported_extension() {
        let data = create_mock_file("FFD8FF", 10);
        let result = FileValidator::validate_image(&data, "test.exe".into(), "application/octet-stream".into());
        assert!(matches!(result, Err(FileValidationError::UnsupportedFileType)));
    }

    /// 测试文件头与扩展名不匹配的情况
    #[test]
    fn test_invalid_header_mismatch() {
        // 后缀是 png，但文件头给的是 JPG 的
        let data = create_mock_file("FFD8FF", 10);
        let result = FileValidator::validate_image(&data, "test.png".into(), "image/png".into());
        assert!(matches!(result, Err(FileValidationError::InvalidHeader)));
    }

    /// 测试空文件名的处理
    #[test]
    fn test_empty_file_name() {
        let data = create_mock_file("FFD8FF", 10);
        let result = FileValidator::validate_image(&data, "".into(), "image/jpeg".into());
        assert!(matches!(result, Err(FileValidationError::EmptyFileName)));
    }

    /// 测试文件太小无法读取完整文件头的情况
    #[test]
    fn test_file_too_small_for_header() {
        // 文件太小，无法读取完整的文件头
        let small_data = vec![0xFFu8; 2]; // 只有 2 字节，不够 4 字节
        let result = FileValidator::validate_image(&small_data, "test.jpg".into(), "image/jpeg".into());
        assert!(matches!(result, Err(FileValidationError::InvalidHeader)));
    }

    // 注意：extract_image_metadata 依赖真正的图片解码，
    // 所以你需要准备一个极其微小的真实图片 Base64 或者 字节数组进行"真图片"测试。

    /// 测试真实 PNG 图片的完整解析流程
    #[test]
    fn test_valid_image_parsing() {
        // 一个 1x1 像素的红色 PNG 字节流
        let tiny_png = hex::decode("89504E470D0A1A0A0000000D4948445200000001000000010802000000907753DE0000000C4944415408D763F8FF7F0005FE02FE0DC444830000000049454E44AE426082").unwrap();
        
        let result = FileValidator::validate_image(
            &tiny_png,
            "pixel.png".into(),
            "image/png".into()
        );
        
        assert!(result.is_ok());
        let meta = result.unwrap();
        assert_eq!(meta.width, 1);
        assert_eq!(meta.height, 1);
        assert_eq!(meta.format, "png");
    }

    /// 测试 JPEG 文件头验证（非真实图片）
    #[test]
    fn test_valid_jpeg_parsing() {
        // 一个最小的有效 JPEG 文件头（实际 JPEG 需要更多内容，但这里只测试文件头验证）
        let jpeg_data = create_mock_file("FFD8FFE0", 100);
        
        // 由于这不是真正的 JPEG，图片解析会失败，但文件头验证应该通过
        let result = FileValidator::validate_image(
            &jpeg_data,
            "test.jpg".into(),
            "image/jpeg".into()
        );
        
        // 文件头验证应该通过，但图片解析可能失败
        match result {
            Ok(_) => {}, // 如果图片解析成功也可以
            Err(FileValidationError::ParseError(_)) => {}, // 图片解析失败是预期的
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    /// 测试 PNG 文件头验证
    #[test]
    fn test_valid_png_header() {
        // PNG 文件头
        let png_data = create_mock_file("89504E47", 100);
        
        let result = FileValidator::validate_image(
            &png_data,
            "test.png".into(),
            "image/png".into()
        );
        
        // 文件头验证应该通过
        match result {
            Ok(_) => {},
            Err(FileValidationError::ParseError(_)) => {}, // 图片解析失败是预期的
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    /// 测试 GIF 文件头验证
    #[test]
    fn test_valid_gif_header() {
        // GIF 文件头 (GIF8)
        let gif_data = create_mock_file("47494638", 100);
        
        let result = FileValidator::validate_image(
            &gif_data,
            "test.gif".into(),
            "image/gif".into()
        );
        
        // 文件头验证应该通过
        match result {
            Ok(_) => {},
            Err(FileValidationError::ParseError(_)) => {},
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    /// 测试 BMP 文件头验证
    #[test]
    fn test_valid_bmp_header() {
        // BMP 文件头 (BM)
        let bmp_data = create_mock_file("424D", 100);
        
        let result = FileValidator::validate_image(
            &bmp_data,
            "test.bmp".into(),
            "image/bmp".into()
        );
        
        // 文件头验证应该通过
        match result {
            Ok(_) => {},
            Err(FileValidationError::ParseError(_)) => {},
            _ => panic!("Unexpected error: {:?}", result),
        }
    }

    /// 测试文件扩展名大小写不敏感
    #[test]
    fn test_case_insensitive_extension() {
        // 测试扩展名大小写不敏感
        let data = create_mock_file("FFD8FF", 100);
        
        let result = FileValidator::validate_image(
            &data,
            "test.JPG".into(),
            "image/jpeg".into()
        );
        
        match result {
            Ok(_) => {},
            Err(FileValidationError::ParseError(_)) => {},
            _ => panic!("Unexpected error: {:?}", result),
        }
    }
}