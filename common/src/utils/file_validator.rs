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

    #[test]
    fn test_bytes_to_hex() {
        let bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(FileValidator::bytes_to_hex(&bytes), "FFD8FFE0");
    }

    #[test]
    fn test_extract_file_extension() {
        assert_eq!(FileValidator::extract_file_extension("test.jpg"), "jpg");
        assert_eq!(FileValidator::extract_file_extension("test.JPG"), "jpg");
        assert_eq!(FileValidator::extract_file_extension("test"), "");
    }

    #[test]
    fn test_is_supported_format() {
        assert!(FileValidator::is_supported_format("jpg"));
        assert!(FileValidator::is_supported_format("png"));
        assert!(!FileValidator::is_supported_format("webp"));
    }
}