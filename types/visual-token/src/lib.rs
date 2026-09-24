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
//! 明文序列化由 serde 派生(加密见 [`VisualTokenStr`]):unit 类型为字符串,
//! `Crop` 作为携带数据的变体嵌套输出,形如:
//! ```json
//! { "fileId": "...", "kind": "Image", "tokenType": "thumbnail", "viewerId": 1 }
//! { "fileId": "...", "kind": "Image", "tokenType": { "crop": { "bbox": ..., "sourceDimensions": ... } }, "viewerId": 1 }
//! ```
//! 明文形态并非对外契约(对外为加密字符串),由服务端自产自销。

use common_core::ContextualResult;
use common_crypto::token_cipher;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    ///
    /// # 参数
    /// - `width`: 原图宽度(像素)
    /// - `height`: 原图高度(像素)
    ///
    /// # 返回
    /// 返回 `(x, y, w, h)`,其中 `(x, y)` 为左上角坐标,`w`/`h` 为裁剪宽高
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
    pub fn decrypt(token: &str) -> ContextualResult<Self> {
        token_cipher().decrypt(token)
    }

    pub fn encrypt(&self) -> ContextualResult<String> {
        // nonce seed 纳入 viewer_id:同一文件的 token 按浏览者区分,
        // 避免"相同 nonce 加密不同明文"导致的 AES-GCM nonce 复用
        let seed = format!("{}:{}", self.viewer_id, self.file_id);
        token_cipher().encrypt(self, Some(&seed))
    }

    /// 创建图片缩略图 token
    ///
    /// # 参数
    /// - `viewer_id`: 浏览者用户 ID
    /// - `file_id`: 文件 ID
    pub fn image_thumbnail(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Image,
            token_type: VisualTokenType::Thumbnail,
            viewer_id: viewer,
        }
    }

    /// 创建图片预览 token
    ///
    /// # 参数
    /// - `viewer_id`: 浏览者用户 ID
    /// - `file_id`: 文件 ID
    pub fn image_preview(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Image,
            token_type: VisualTokenType::Preview,
            viewer_id: viewer,
        }
    }

    /// 创建视频缩略图 token(封面截帧)
    ///
    /// # 参数
    /// - `viewer_id`: 浏览者用户 ID
    /// - `file_id`: 文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Thumbnail`、`kind = Video` 的 `VisualToken`。
    /// 下载时由服务端按视频时长中点截帧。
    pub fn video_thumbnail(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Video,
            token_type: VisualTokenType::Thumbnail,
            viewer_id: viewer,
        }
    }

    /// 创建视频预览 token(封面截帧)
    ///
    /// # 参数
    /// - `viewer_id`: 浏览者用户 ID
    /// - `file_id`: 文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Preview`、`kind = Video` 的 `VisualToken`。
    /// 下载时由服务端按视频时长中点截帧。
    pub fn video_preview(viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind: VisualKind::Video,
            token_type: VisualTokenType::Preview,
            viewer_id: viewer,
        }
    }

    /// 创建原图/原视频 token
    ///
    /// # 参数
    /// - `kind`: 视觉类型(图片原图或视频原流)
    /// - `viewer_id`: 浏览者用户 ID
    /// - `file_id`: 文件 ID
    ///
    /// # 返回
    /// 返回类型为 `Original` 的 `VisualToken`
    pub fn original(kind: VisualKind, viewer: UserId, file_id: impl Into<String>) -> Self {
        Self {
            file_id: file_id.into(),
            kind,
            token_type: VisualTokenType::Original,
            viewer_id: viewer,
        }
    }

    /// 创建裁剪图 token(人脸封面,仅图片)
    ///
    /// # 参数
    /// - `file_id`: 文件 ID
    /// - `bbox`: 人脸边界框(归一化坐标),用于定位裁剪区域
    /// - `viewer_id`: 浏览者用户 ID
    ///
    /// # 返回
    /// 返回类型为 `Crop` 且携带裁剪信息的 `VisualToken`
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

/// 自动加密的视觉 token 封装
///
/// 序列化时自动输出加密字符串,反序列化只接受加密字符串并解密还原。
/// 内部持有 [`VisualToken`] 纯数据,适合作为 DTO 字段类型(如 `avatar_token`)。
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(type = "string"))]
pub struct VisualTokenStr(pub VisualToken);

impl VisualTokenStr {
    /// 取出内部 token(解密后的原始数据)
    pub fn into_inner(self) -> VisualToken {
        self.0
    }
}

impl Serialize for VisualTokenStr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let token = self.0.encrypt().map_err(serde::ser::Error::custom)?;

        serializer.serialize_str(&token)
    }
}

impl<'de> Deserialize<'de> for VisualTokenStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 只接受加密字符串(不接受明文对象),反序列化即解密
        let token = String::deserialize(deserializer)?;

        VisualToken::decrypt(&token)
            .map(VisualTokenStr)
            .map_err(serde::de::Error::custom)
    }
}

impl From<VisualToken> for VisualTokenStr {
    fn from(token: VisualToken) -> Self {
        Self(token)
    }
}

impl From<VisualTokenStr> for VisualToken {
    fn from(wrapper: VisualTokenStr) -> Self {
        wrapper.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_crypto::{TokenCipherConfig, init_token_cipher};

    fn init_test_cipher() {
        init_token_cipher(&TokenCipherConfig {
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
        init_test_cipher();
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
        init_test_cipher();
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
    fn test_visual_token_str_serializes_to_encrypted_string() {
        init_test_cipher();
        let token = VisualToken::image_preview(UserId(5), "img-1");
        let json = serde_json::to_value(VisualTokenStr::from(token)).unwrap();
        assert!(
            json.is_string(),
            "VisualTokenStr 应序列化为加密字符串, 实际为 {json}"
        );
        // 加密字符串可解密还原全部字段
        let decrypted: VisualTokenStr = serde_json::from_value(json).unwrap();
        assert_eq!(decrypted.0.file_id, "img-1");
        assert_eq!(decrypted.0.kind, VisualKind::Image);
        assert_eq!(decrypted.0.token_type, VisualTokenType::Preview);
        assert_eq!(decrypted.0.viewer_id, UserId(5));
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
#[cfg(test)]
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
