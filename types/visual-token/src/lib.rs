//! 统一视觉 Token(图片 / 视频)
//!
//! 跨上下文协议 crate:提供视觉访问 token 契约类型(`VisualToken` /
//! `VisualTokenType` / `FaceBBox` / `ImageDimensions`),供 visual / user(身份)
//! 两个上下文复用,并可随 DTO 一起导出 TS。
//!
//! 位于 `types-core`(强类型 ID / `VisualKind`)之上、各域契约 crate 之下 ——
//! `visual_token` 的 `kind` 字段引用 `VisualKind`,所以该枚举必须留在内核,
//! 而本 crate 不能反过来依赖 visual 域契约。
//!
//! 明文序列化由 serde 派生；对外契约是[`VisualTokenStr`](不透明字符串)。
//! unit 类型为字符串, `Crop` 作为携带数据的变体嵌套输出,形如:
//! ```json
//! { "fileId": "...", "kind": "Image", "tokenType": "thumbnail", "viewerId": 1 }
//! { "fileId": "...", "kind": "Image", "tokenType": { "crop": { "bbox": ..., "sourceDimensions": ... } }, "viewerId": 1 }
//! ```
//! 明文形态并非对外契约(对外为不透明字符串),由服务端自产自销。
//! 加解密(`VisualToken::encrypt`/`decrypt` 与 [`VisualTokenStr`] 的同名方法)
//! 仅在 `crypto` feature 下可用。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "crypto")]
use common_core::ContextualResult;
#[cfg(feature = "crypto")]
use common_crypto::token_cipher;

use types_core::{UserId, VisualKind};

/// 视觉 token 类型
///
/// `Crop`(人脸封面)携带裁剪信息,由类型系统保证"仅 Crop 需要
/// bbox / 原图尺寸";其余类型不携带。
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum VisualTokenType {
    Thumbnail,
    Preview,
    Original,
    Crop {
        bbox: FaceBBox,
        #[serde(rename = "sourceDimensions")]
        source_dimensions: ImageDimensions,
    },
}

/// 人脸边界框(归一化坐标,x1/y1 左上角、x2/y2 右下角,取值范围 0~1)
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "visual/", rename = "FaceBBox"))]
pub struct FaceBBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}
#[cfg(feature = "face-engine")]
impl From<insight_face_rs::BoundingBox> for FaceBBox {
    /// 将 insight-face-rs 的边界框转换为归一化 `FaceBBox`
    fn from(v: insight_face_rs::BoundingBox) -> Self {
        Self {
            x1: v.x1,
            y1: v.y1,
            x2: v.x2,
            y2: v.y2,
        }
    }
}
#[cfg(feature = "face-engine")]
impl From<FaceBBox> for insight_face_rs::BoundingBox {
    /// 将归一化 `FaceBBox` 转换为 insight-face-rs 的边界框
    fn from(v: FaceBBox) -> Self {
        Self {
            x1: v.x1,
            y1: v.y1,
            x2: v.x2,
            y2: v.y2,
        }
    }
}

/// 原图尺寸(像素)。
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "visual/", rename = "ImageDimensions")
)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}
impl FaceBBox {
    /// 将归一化边界框转换为指定图片尺寸下的绝对像素裁剪矩形 `(x, y, w, h)`
    pub fn to_pixel_rect(self, width: u32, height: u32) -> (i32, i32, i32, i32) {
        let w = width as f32;
        let h = height as f32;
        let x = (self.x1 * w).round().clamp(0.0, w) as i32;
        let y = (self.y1 * h).round().clamp(0.0, h) as i32;
        let x2 = (self.x2 * w).round().clamp(x as f32, w) as i32;
        let y2 = (self.y2 * h).round().clamp(y as f32, h) as i32;
        (x, y, x2 - x, y2 - y)
    }
}

/// 统一视觉 Token(纯数据,序列化为明文对象;需要加密时用 [`VisualTokenStr`])
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VisualToken {
    /// 文件路径
    pub file_id: String,
    /// 视觉类型:图片 / 视频
    pub kind: VisualKind,
    /// 视觉类型
    pub token_type: VisualTokenType,
    /// 浏览者用户 ID(视觉访问审计主体)
    pub viewer_id: UserId,
}

impl VisualToken {
    /// 从加密字符串解密出视觉令牌(需启用 `crypto` feature)
    #[cfg(feature = "crypto")]
    pub fn decrypt(token: &str) -> ContextualResult<Self> {
        token_cipher().decrypt(token)
    }

    /// 将视觉令牌加密为字符串；nonce seed 纳入 `viewer_id`,避免 nonce 复用(需启用 `crypto` feature)
    #[cfg(feature = "crypto")]
    pub fn encrypt(&self) -> ContextualResult<String> {
        // nonce seed 纳入 viewer_id:同一文件的 token 按浏览者区分,
        // 避免"相同 nonce 加密不同明文"导致的 AES-GCM nonce 复用
        let seed = format!("{}:{}", self.viewer_id, self.file_id);
        token_cipher().encrypt(self, Some(&seed))
    }

    /// 创建图片缩略图 token
    pub fn image_thumbnail(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Image,
            token_type: VisualTokenType::Thumbnail,
            viewer_id: viewer,
        }
    }

    /// 创建图片预览 token
    pub fn image_preview(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Image,
            token_type: VisualTokenType::Preview,
            viewer_id: viewer,
        }
    }

    /// 创建视频缩略图 token(封面截帧)
    pub fn video_thumbnail(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Video,
            token_type: VisualTokenType::Thumbnail,
            viewer_id: viewer,
        }
    }

    /// 创建视频预览 token(封面截帧)
    pub fn video_preview(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Video,
            token_type: VisualTokenType::Preview,
            viewer_id: viewer,
        }
    }

    /// 创建原图/原视频 token
    pub fn original(kind: VisualKind, viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind,
            token_type: VisualTokenType::Original,
            viewer_id: viewer,
        }
    }

    /// 创建裁剪图 token(人脸封面,仅图片)
    pub fn crop(
        viewer: UserId,
        file_id: impl Into<String>,
        bbox: FaceBBox,
        source_dimensions: ImageDimensions,
    ) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Image,
            token_type: VisualTokenType::Crop {
                bbox,
                source_dimensions,
            },
            viewer_id: viewer,
        }
    }
}

/// 视觉 token 的“字符串封装”。
///
/// 在 JSON 上就是一个**不透明字符串**:无论序列化还是反序列化都**不会**触发
/// 加解密,因此不持有密钥的消费方(如外部客户端)也能安全解析。
///
/// 加解密是持有密钥方(服务端)的显式操作,需启用 `crypto` feature:
/// - [`VisualTokenStr::encrypt`]：由明文令牌生成
/// - [`VisualTokenStr::decrypt`]：还原明文令牌
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(type = "string"))]
pub struct VisualTokenStr(String);

impl VisualTokenStr {
    /// 以已加密字符串构造(不对内容做任何校验/解密)。
    pub fn new(encrypted: impl Into<String>) -> Self {
        Self(encrypted.into())
    }

    /// 取得加密字符串。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 将明文令牌加密为字符串封装(持密钥方,需 `crypto` feature)。
    #[cfg(feature = "crypto")]
    pub fn encrypt(token: &VisualToken) -> ContextualResult<Self> {
        token.encrypt().map(Self)
    }

    /// 解密还原明文令牌(持密钥方,需 `crypto` feature)。
    #[cfg(feature = "crypto")]
    pub fn decrypt(&self) -> ContextualResult<VisualToken> {
        VisualToken::decrypt(&self.0)
    }
}

impl Serialize for VisualTokenStr {
    /// 原样输出字符串(不加密)
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for VisualTokenStr {
    /// 原样读入字符串(不解密),外部客户端无需密钥
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self)
    }
}

impl From<String> for VisualTokenStr {
    /// 从已加密字符串构造
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "crypto")]
    fn init_test_cipher() {
        common_crypto::init_token_cipher(&common_crypto::TokenCipherConfig {
            key: "test-key-for-unit-tests".to_owned(),
            salt: "test-salt".to_owned(),
        });
    }

    #[test]
    fn test_image_thumbnail_constructor() {
        let token = VisualToken::image_thumbnail(UserId(7), "abc123");
        assert_eq!(token.file_id, "abc123");
        assert_eq!(token.kind, VisualKind::Image);
        assert_eq!(token.token_type, VisualTokenType::Thumbnail);
        assert_eq!(token.viewer_id, UserId(7));
    }

    #[test]
    fn test_image_thumbnail_accepts_string() {
        let token = VisualToken::image_thumbnail(UserId(1), String::from("file-001"));
        assert_eq!(token.file_id, "file-001");
        assert_eq!(token.token_type, VisualTokenType::Thumbnail);
    }

    #[test]
    fn test_video_thumbnail_constructor() {
        let token = VisualToken::video_thumbnail(UserId(8), "video-001");
        assert_eq!(token.file_id, "video-001");
        assert_eq!(token.kind, VisualKind::Video);
        assert_eq!(token.token_type, VisualTokenType::Thumbnail);
        assert_eq!(token.viewer_id, UserId(8));
    }

    #[test]
    fn test_video_preview_constructor() {
        let token = VisualToken::video_preview(UserId(9), "video-002");
        assert_eq!(token.kind, VisualKind::Video);
        assert_eq!(token.token_type, VisualTokenType::Preview);
    }

    #[test]
    fn test_image_preview_constructor() {
        let token = VisualToken::image_preview(UserId(2), "preview-id");
        assert_eq!(token.file_id, "preview-id");
        assert_eq!(token.kind, VisualKind::Image);
        assert_eq!(token.token_type, VisualTokenType::Preview);
    }

    #[test]
    fn test_original_constructor() {
        let token = VisualToken::original(VisualKind::Image, UserId(3), "original-id");
        assert_eq!(token.file_id, "original-id");
        assert_eq!(token.kind, VisualKind::Image);
        assert_eq!(token.token_type, VisualTokenType::Original);
        let video_token = VisualToken::original(VisualKind::Video, UserId(3), "video-id");
        assert_eq!(video_token.kind, VisualKind::Video);
    }

    #[test]
    fn test_crop_with_bbox() {
        let bbox = FaceBBox {
            x1: 0.1,
            y1: 0.2,
            x2: 0.6,
            y2: 0.9,
        };
        let dimensions = ImageDimensions {
            width: 800,
            height: 400,
        };
        let token = VisualToken::crop(UserId(4), "crop-id", bbox, dimensions);
        assert_eq!(token.file_id, "crop-id");
        assert_eq!(token.kind, VisualKind::Image);
        match token.token_type {
            VisualTokenType::Crop {
                bbox: b,
                source_dimensions,
            } => {
                assert_eq!(b.x1, 0.1);
                assert_eq!(b.y1, 0.2);
                assert_eq!(b.x2, 0.6);
                assert_eq!(b.y2, 0.9);
                assert_eq!(source_dimensions, dimensions);
            }
            other => panic!("Crop 构造器应得到 Crop 变体, 实际为 {other:?}"),
        }
    }

    #[test]
    fn test_visual_token_serialize_roundtrip() {
        let token = VisualToken::image_thumbnail(UserId(11), "file-abc");
        let json = serde_json::to_string(&token).unwrap();
        let deserialized: VisualToken = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_id, "file-abc");
        assert_eq!(deserialized.kind, VisualKind::Image);
        assert_eq!(deserialized.token_type, VisualTokenType::Thumbnail);
        assert_eq!(deserialized.viewer_id, UserId(11));
    }

    #[test]
    fn test_visual_token_plain_serializes_to_object() {
        let token = VisualToken::image_preview(UserId(5), "img-1");
        let json = serde_json::to_value(&token).unwrap();
        assert!(json.is_object());
        assert_eq!(json["tokenType"], "preview");
        assert_eq!(json["kind"], "Image");
        assert_eq!(json["fileId"], "img-1");
        assert_eq!(json["viewerId"], "5");
    }

    #[test]
    fn test_video_token_serialize_roundtrip() {
        let token = VisualToken::video_thumbnail(UserId(13), "video-x");
        let json = serde_json::to_string(&token).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["kind"], "Video");
        assert_eq!(value["tokenType"], "thumbnail");
        let deserialized: VisualToken = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.kind, VisualKind::Video);
    }

    #[test]
    fn test_crop_serialize_roundtrip() {
        let bbox = FaceBBox {
            x1: 0.05,
            y1: 0.1,
            x2: 0.55,
            y2: 0.7,
        };
        let dimensions = ImageDimensions {
            width: 800,
            height: 400,
        };
        let token = VisualToken::crop(UserId(12), "file-xyz", bbox, dimensions);
        let json = serde_json::to_string(&token).unwrap();
        // Crop 作为携带数据的变体,数据嵌套在 tokenType 内
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["tokenType"]["crop"]["bbox"]["x1"], 0.05);
        assert_eq!(value["tokenType"]["crop"]["sourceDimensions"]["width"], 800);
        let deserialized: VisualToken = serde_json::from_str(&json).unwrap();
        match deserialized.token_type {
            VisualTokenType::Crop {
                bbox: b,
                source_dimensions,
            } => {
                assert_eq!(b.x1, 0.05);
                assert_eq!(source_dimensions, dimensions);
            }
            other => panic!("roundtrip 后应为 Crop 变体, 实际为 {other:?}"),
        }
        assert_eq!(deserialized.viewer_id, UserId(12));
    }

    #[test]
    fn test_token_without_kind_fails_deserialize() {
        // kind 为必填字段,缺 kind 应反序列化失败
        let json = r#"{"fileId":"file-abc","tokenType":"preview","viewerId":1}"#;
        let result: Result<VisualToken, _> = serde_json::from_str(json);
        assert!(result.is_err(), "缺 kind 字段应反序列化失败");
    }

    #[test]
    fn test_crop_without_bbox_fails_deserialize() {
        // Crop 变体缺 bbox 字段应反序列化失败
        let json = r#"{"fileId":"file-abc","kind":"Image","tokenType":{"crop":{"sourceDimensions":{"width":800,"height":400}}},"viewerId":1}"#;
        let result: Result<VisualToken, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Crop 缺 bbox 应反序列化失败");
    }

    #[test]
    fn test_unknown_token_type_fails_deserialize() {
        let json = r#"{"fileId":"file-abc","kind":"Image","tokenType":"unknown","viewerId":1}"#;
        let result: Result<VisualToken, _> = serde_json::from_str(json);
        assert!(result.is_err(), "未知 type 应反序列化失败");
    }

    #[test]
    fn test_missing_viewer_fails_deserialize() {
        let json = r#"{"fileId":"file-abc","type":"preview"}"#;
        let result: Result<VisualToken, _> = serde_json::from_str(json);
        assert!(result.is_err(), "缺 viewerId 的旧 token 应反序列化失败");
    }

    #[test]
    fn test_visual_token_str_is_opaque_string_without_cipher() {
        // 无须初始化 cipher:反序列化不触发解密
        let token: VisualTokenStr = serde_json::from_value(serde_json::json!("opaque")).unwrap();
        assert_eq!(token.as_str(), "opaque");
        // 序列化原样输出
        assert_eq!(
            serde_json::to_value(&token).unwrap(),
            serde_json::json!("opaque")
        );
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn test_visual_token_str_encrypt_decrypt_roundtrip() {
        init_test_cipher();
        let plain = VisualToken::image_preview(UserId(5), "img-1");
        let wrapped = VisualTokenStr::encrypt(&plain).unwrap();
        // 对外是不透明字符串
        let json = serde_json::to_value(&wrapped).unwrap();
        assert!(json.is_string(), "应序列化为字符串, 实际为 {json}");
        // 反序列化(不解密)
        let parsed: VisualTokenStr = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.as_str(), wrapped.as_str());
        // 持密钥方可显式解密还原
        let decrypted = parsed.decrypt().unwrap();
        assert_eq!(decrypted.file_id, "img-1");
        assert_eq!(decrypted.kind, VisualKind::Image);
        assert_eq!(decrypted.token_type, VisualTokenType::Preview);
        assert_eq!(decrypted.viewer_id, UserId(5));
    }

    #[test]
    fn test_face_bbox_to_pixel_rect() {
        let bbox = FaceBBox {
            x1: 0.1,
            y1: 0.25,
            x2: 0.6,
            y2: 1.0,
        };
        // 800x400 图片:x1=80, y1=100, x2=480, y2=400
        let (x, y, w, h) = bbox.to_pixel_rect(800, 400);
        assert_eq!(x, 80);
        assert_eq!(y, 100);
        assert_eq!(w, 400);
        assert_eq!(h, 300);
    }

    #[test]
    fn test_face_bbox_to_pixel_rect_clamps_bounds() {
        let bbox = FaceBBox {
            x1: -0.5,
            y1: 0.0,
            x2: 1.5,
            y2: 1.0,
        };
        let (x, y, w, h) = bbox.to_pixel_rect(200, 100);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
        assert_eq!(w, 200);
        assert_eq!(h, 100);
    }
}

/// 加解密往返测试。
///
/// 原先挂在 `types` 的 `orm` feature 下, 但这些测试并不依赖 sea-orm,
/// 拆分为独立 crate 后改为无条件运行。
#[cfg(all(test, feature = "crypto"))]
mod cipher_roundtrip_tests {
    use super::*;
    use common_crypto::{TokenCipherConfig, init_token_cipher};

    fn test_cipher() -> &'static common_crypto::TokenCipher {
        init_token_cipher(&TokenCipherConfig {
            key: "test-key-for-unit-tests".to_owned(),
            salt: "test-salt".to_owned(),
        })
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        test_cipher();
        let token = VisualToken::image_thumbnail(UserId(9), "avatar-file-id")
            .encrypt()
            .unwrap();
        // 验证能解密回来
        let decrypted: VisualToken = VisualToken::decrypt(&token).unwrap();
        assert_eq!(decrypted.file_id, "avatar-file-id");
        assert_eq!(decrypted.kind, VisualKind::Image);
        assert_eq!(decrypted.token_type, VisualTokenType::Thumbnail);
        assert_eq!(decrypted.viewer_id, UserId(9));
    }

    #[test]
    fn test_video_encrypt_decrypt_roundtrip() {
        test_cipher();
        let token = VisualToken::video_thumbnail(UserId(10), "video-file-id")
            .encrypt()
            .unwrap();
        let decrypted: VisualToken = VisualToken::decrypt(&token).unwrap();
        assert_eq!(decrypted.kind, VisualKind::Video);
        assert_eq!(decrypted.token_type, VisualTokenType::Thumbnail);
    }
}
