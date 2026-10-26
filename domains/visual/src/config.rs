use std::path::PathBuf;

use serde::Deserialize;

/// 影像域配置
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VisualConfig {
    /// 图片处理后端选择: auto / oss / local
    #[serde(default)]
    pub image_processor: ImageProcessor,

    /// 处理后的图片存储配置
    #[serde(default)]
    pub processed_image: ProcessedImageConfig,

    /// 视频转码配置(生成缩略/预览衍生片)
    #[serde(default)]
    pub transcode: TranscodeConfig,
}

/// 图片处理后端
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageProcessor {
    /// 依对象存储端点自动判定: 阿里云 OSS 走服务端处理, 否则本地处理
    #[default]
    Auto,
    /// 强制使用对象存储的图片处理(x-oss-process, 仅阿里云 OSS 支持)
    Oss,
    /// 强制使用本地图片处理
    Local,
}

impl ImageProcessor {
    /// 依对象存储端点解析最终生效的图片处理后端；`auto` 时端点为阿里云 OSS(`aliyuncs.com`)则用 `oss`, 否则用 `local`
    pub fn resolve(self, endpoint: &str) -> ResolvedImageProcessor {
        match self {
            Self::Oss => ResolvedImageProcessor::Oss,
            Self::Local => ResolvedImageProcessor::Local,
            Self::Auto if endpoint.contains("aliyuncs.com") => ResolvedImageProcessor::Oss,
            Self::Auto => ResolvedImageProcessor::Local,
        }
    }
}

/// 解析后最终生效的图片处理后端(不含 auto)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedImageProcessor {
    Oss,
    Local,
}

/// 处理后的图片存储配置
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct ProcessedImageConfig {
    /// 是否把缩略图/预览图持久化到对象存储；false 时每次请求按处理后端直接处理。
    /// 裁剪图不参与持久化，始终直接处理。
    #[serde(default)]
    pub store_in_oss: bool,
}

/// 视频转码配置
#[derive(Debug, Clone, Deserialize)]
pub struct TranscodeConfig {
    /// ffmpeg 可执行文件路径(或 PATH 中的命令名)
    #[serde(default = "default_ffmpeg_path")]
    pub ffmpeg_path: PathBuf,

    /// 缩略片时长(秒, 从片头开始取)
    #[serde(default = "default_thumb_duration_secs")]
    pub thumb_duration_secs: u32,

    /// 缩略片目标宽度(像素, 仅缩不放)
    #[serde(default = "default_thumb_max_width")]
    pub thumb_max_width: u32,

    /// 预览片目标高度(像素, 仅缩不放)
    #[serde(default = "default_preview_max_height")]
    pub preview_max_height: u32,

    /// SVT-AV1 CRF(0-63, 越大体积越小/画质越低)
    #[serde(default = "default_av1_crf")]
    pub av1_crf: u8,

    /// SVT-AV1 preset(0-13, 越大编码越快/压缩率越低)
    #[serde(default = "default_av1_preset")]
    pub av1_preset: u8,

    /// 单个衍生片转码失败后的最大尝试次数(含首次); 超过后置为终态 failed
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,

    /// 转码失败后的重试退避(秒)
    #[serde(default = "default_retry_backoff_secs")]
    pub retry_backoff_secs: u64,
}

impl TranscodeConfig {
    /// 校验转码配置, 非法时返回原因字符串
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.thumb_duration_secs == 0 {
            return Err("thumb_duration_secs 必须大于 0".to_string());
        }
        if self.thumb_max_width == 0 {
            return Err("thumb_max_width 必须大于 0".to_string());
        }
        if self.preview_max_height == 0 {
            return Err("preview_max_height 必须大于 0".to_string());
        }
        if self.max_attempts == 0 {
            return Err("max_attempts 必须大于 0".to_string());
        }
        if self.av1_crf > 63 {
            return Err(format!("av1_crf 必须在 0-63 之间, 当前为 {}", self.av1_crf));
        }
        if self.av1_preset > 13 {
            return Err(format!(
                "av1_preset 必须在 0-13 之间, 当前为 {}",
                self.av1_preset
            ));
        }
        Ok(())
    }
}

impl Default for TranscodeConfig {
    /// 返回转码配置的默认值
    fn default() -> Self {
        Self {
            ffmpeg_path: default_ffmpeg_path(),
            thumb_duration_secs: default_thumb_duration_secs(),
            thumb_max_width: default_thumb_max_width(),
            preview_max_height: default_preview_max_height(),
            av1_crf: default_av1_crf(),
            av1_preset: default_av1_preset(),
            max_attempts: default_max_attempts(),
            retry_backoff_secs: default_retry_backoff_secs(),
        }
    }
}

/// 默认 ffmpeg 命令
fn default_ffmpeg_path() -> PathBuf {
    PathBuf::from("ffmpeg")
}
/// 默认缩略片时长 4 秒
fn default_thumb_duration_secs() -> u32 {
    4
}
/// 默认缩略片宽度 640
fn default_thumb_max_width() -> u32 {
    640
}
/// 默认预览片高度 1080
fn default_preview_max_height() -> u32 {
    1080
}
/// 默认 SVT-AV1 CRF
fn default_av1_crf() -> u8 {
    35
}
/// 默认 SVT-AV1 preset
fn default_av1_preset() -> u8 {
    8
}
/// 默认最大尝试次数
fn default_max_attempts() -> u32 {
    3
}
/// 默认重试退避秒数
fn default_retry_backoff_secs() -> u64 {
    30
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_processor_auto_resolves_by_endpoint() {
        assert_eq!(
            ImageProcessor::Auto.resolve("https://oss-cn-hangzhou.aliyuncs.com"),
            ResolvedImageProcessor::Oss
        );
        assert_eq!(
            ImageProcessor::Auto.resolve("http://localhost:9000"),
            ResolvedImageProcessor::Local
        );
    }

    #[test]
    fn image_processor_forced_overrides_endpoint() {
        assert_eq!(
            ImageProcessor::Oss.resolve("http://localhost:9000"),
            ResolvedImageProcessor::Oss
        );
        assert_eq!(
            ImageProcessor::Local.resolve("https://oss-cn-hangzhou.aliyuncs.com"),
            ResolvedImageProcessor::Local
        );
    }

    #[test]
    fn processed_image_defaults_to_direct_processing() {
        assert!(!VisualConfig::default().processed_image.store_in_oss);
    }

    #[test]
    fn transcode_defaults_are_stable() {
        let cfg = TranscodeConfig::default();
        assert_eq!(cfg.thumb_duration_secs, 4);
        assert_eq!(cfg.thumb_max_width, 640);
        assert_eq!(cfg.preview_max_height, 1080);
        assert_eq!(cfg.av1_crf, 35);
        assert_eq!(cfg.av1_preset, 8);
    }

    #[test]
    fn transcode_validate_accepts_defaults() {
        assert!(TranscodeConfig::default().validate().is_ok());
    }

    #[test]
    fn transcode_validate_rejects_bad_av1_settings() {
        let crf = TranscodeConfig {
            av1_crf: 64,
            ..TranscodeConfig::default()
        };
        assert!(crf.validate().is_err());

        let preset = TranscodeConfig {
            av1_preset: 14,
            ..TranscodeConfig::default()
        };
        assert!(preset.validate().is_err());
    }
}
