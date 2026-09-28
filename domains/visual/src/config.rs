use std::path::PathBuf;

use serde::Deserialize;

/// 影像域配置
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VisualConfig {
    /// 图片处理后端选择: auto / oss / local
    #[serde(default)]
    pub image_processor: ImageProcessor,

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
    /// 依对象存储端点解析最终生效的后端.
    ///
    /// `auto` 时: 端点为阿里云 OSS(`aliyuncs.com`)则用 `oss`, 否则用 `local`。
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

/// 视频转码配置
#[derive(Debug, Clone, Deserialize)]
pub struct TranscodeConfig {
    /// 是否启用视频转码; 关闭时不生成衍生片
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// ffmpeg 可执行文件路径(或 PATH 中的命令名)
    #[serde(default = "default_ffmpeg_path")]
    pub ffmpeg_path: PathBuf,

    /// 缩略片时长(秒, 从片头开始取)
    #[serde(default = "default_thumb_duration_secs")]
    pub thumb_duration_secs: u32,

    /// 缩略片目标宽度(像素, 仅缩不放)
    #[serde(default = "default_thumb_max_width")]
    pub thumb_max_width: u32,

    /// 缩略片视频码率
    #[serde(default = "default_thumb_bitrate")]
    pub thumb_bitrate: String,

    /// 预览片目标高度(像素, 仅缩不放)
    #[serde(default = "default_preview_max_height")]
    pub preview_max_height: u32,

    /// 预览片视频码率
    #[serde(default = "default_preview_bitrate")]
    pub preview_bitrate: String,

    /// 单个衍生片转码失败后的最大尝试次数(含首次); 超过后置为终态 failed
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,

    /// 转码失败后的重试退避(秒)
    #[serde(default = "default_retry_backoff_secs")]
    pub retry_backoff_secs: u64,
}

impl TranscodeConfig {
    /// 校验配置; `Err(原因)` 表示非法(仅在 `enabled` 时校验有意义的字段)。
    pub fn validate(&self) -> std::result::Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
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
        if !is_valid_bitrate(&self.thumb_bitrate) {
            return Err(format!(
                "thumb_bitrate 非法: {}(示例: 600k / 4M / 1200000)",
                self.thumb_bitrate
            ));
        }
        if !is_valid_bitrate(&self.preview_bitrate) {
            return Err(format!(
                "preview_bitrate 非法: {}(示例: 600k / 4M / 1200000)",
                self.preview_bitrate
            ));
        }
        Ok(())
    }
}

/// 校验码率字符串: 正数, 可带 k/K/m/M/g/G 后缀。
fn is_valid_bitrate(value: &str) -> bool {
    let value = value.trim();
    let bytes = value.as_bytes();
    let (num, suffix) = match bytes.last() {
        Some(c) if c.is_ascii_alphabetic() => (&value[..value.len() - 1], Some(*c)),
        _ => (value, None),
    };
    let number_ok = num.parse::<f64>().is_ok_and(|n| n > 0.0);
    let suffix_ok = matches!(suffix, None | Some(b'k' | b'K' | b'm' | b'M' | b'g' | b'G'));
    number_ok && suffix_ok
}

impl Default for TranscodeConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            ffmpeg_path: default_ffmpeg_path(),
            thumb_duration_secs: default_thumb_duration_secs(),
            thumb_max_width: default_thumb_max_width(),
            thumb_bitrate: default_thumb_bitrate(),
            preview_max_height: default_preview_max_height(),
            preview_bitrate: default_preview_bitrate(),
            max_attempts: default_max_attempts(),
            retry_backoff_secs: default_retry_backoff_secs(),
        }
    }
}

/// 默认启用视频转码.
fn default_enabled() -> bool {
    true
}
/// 默认 ffmpeg 命令.
fn default_ffmpeg_path() -> PathBuf {
    PathBuf::from("ffmpeg")
}
/// 默认缩略片时长 4 秒.
fn default_thumb_duration_secs() -> u32 {
    4
}
/// 默认缩略片宽度 640.
fn default_thumb_max_width() -> u32 {
    640
}
/// 默认缩略片码率.
fn default_thumb_bitrate() -> String {
    "600k".to_string()
}
/// 默认预览片高度 1080.
fn default_preview_max_height() -> u32 {
    1080
}
/// 默认预览片码率.
fn default_preview_bitrate() -> String {
    "4M".to_string()
}
/// 默认最大尝试次数.
fn default_max_attempts() -> u32 {
    3
}
/// 默认重试退避秒数.
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
    fn transcode_defaults_are_stable() {
        let cfg = TranscodeConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.thumb_duration_secs, 4);
        assert_eq!(cfg.thumb_max_width, 640);
        assert_eq!(cfg.preview_max_height, 1080);
    }

    #[test]
    fn transcode_validate_accepts_defaults() {
        assert!(TranscodeConfig::default().validate().is_ok());
    }

    #[test]
    fn transcode_validate_rejects_bad_bitrate() {
        let cfg = TranscodeConfig {
            thumb_bitrate: "abc".to_string(),
            ..TranscodeConfig::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn transcode_validate_skips_when_disabled() {
        let cfg = TranscodeConfig {
            enabled: false,
            thumb_duration_secs: 0,
            ..TranscodeConfig::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn bitrate_validation_rules() {
        for ok in ["600k", "4M", "1200000", "1.5M", "2g"] {
            assert!(is_valid_bitrate(ok), "{ok} 应合法");
        }
        for bad in ["", "abc", "0", "-1", "4X", "M"] {
            assert!(!is_valid_bitrate(bad), "{bad} 应非法");
        }
    }
}
