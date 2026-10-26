use common_core::time::DateTime;

#[cfg(feature = "orm")]
use crate::VisualToken;
#[cfg(feature = "orm")]
use crate::visual::VisualRecord;
use crate::visual::{VisualId, VisualKind};
use types_core::UserId;
use types_core::cursor::TimeIdCursor;

crate::dto!(VisualTokens, "visual/", rename = "VisualTokens"; {
    pub thumbnail: String,
    pub preview: String,
    pub original: String,
});

crate::dto!(VisualView, "visual/", rename = "Visual"; {
    pub id: VisualId,
    pub user_id: UserId,
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: u64,
    pub created_at: DateTime,
    pub kind: VisualKind,
    /// 视频长度(毫秒, 图片为 0, 此时序列化时省略)。
    ///
    /// 序列化会省略 0 值, 因此反序列化必须允许字段缺失(`default`),
    /// 否则同一类型的输出无法被自己解析。
    #[serde(default, skip_serializing_if = "is_zero")]
    pub duration_ms: u64,
    pub is_liked: bool,
    pub tokens: VisualTokens,
});
/// 判断数值是否为零(用于序列化时省略 0 值)
fn is_zero(num: &u64) -> bool {
    *num == 0
}

impl VisualView {
    /// 写入当前用户对影像的点赞状态
    pub fn with_liked(mut self, is_liked: bool) -> Self {
        self.is_liked = is_liked;
        self
    }
}

#[cfg(feature = "orm")]
impl VisualView {
    /// 由影像记录与查看者构造视图,并生成各类访问令牌
    pub fn from_record_with_tokens(
        record: VisualRecord,
        viewer: UserId,
    ) -> common_core::error::contextual::Result<Self> {
        let kind = record.kind;
        let file_id = record.file_id.clone();

        let tokens = match kind {
            VisualKind::Image => VisualTokens {
                thumbnail: VisualToken::image_thumbnail(viewer, &file_id).encrypt()?,
                preview: VisualToken::image_preview(viewer, &file_id).encrypt()?,
                original: VisualToken::original(VisualKind::Image, viewer, &file_id).encrypt()?,
            },
            VisualKind::Video => VisualTokens {
                thumbnail: VisualToken::video_thumbnail(viewer, &file_id).encrypt()?,
                preview: VisualToken::video_preview(viewer, &file_id).encrypt()?,
                original: VisualToken::original(VisualKind::Video, viewer, &file_id).encrypt()?,
            },
        };

        Ok(Self {
            id: record.id,
            user_id: record.user_id,
            name: record.name,
            width: record.width,
            height: record.height,
            size: record.size,
            created_at: record.created_at,
            kind,
            duration_ms: record.duration_ms,
            is_liked: false,
            tokens,
        })
    }
}

crate::dto!(VisualCursorParam, "visual/", serde_default, docs = "影像游标参数（cursor 为 TimeIdCursor<VisualId> 的 Base64 编码）"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<VisualId>>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
    pub direction: PageDirection,
    pub anchor_time: Option<DateTime>,
});

#[cfg(all(test, feature = "orm"))]
mod orm_tests {
    use super::*;
    use crate::{ImageTokenType, VideoTokenType, VisualTokenKind};
    use common_crypto::{TokenCipher, TokenCipherConfig, init_token_cipher};

    fn test_cipher() -> &'static TokenCipher {
        init_token_cipher(&TokenCipherConfig {
            key: "test-key".to_owned(),
            salt: "test-salt".to_owned(),
        })
    }

    fn visual_record() -> VisualRecord {
        VisualRecord {
            id: VisualId(1),
            user_id: UserId(2),
            name: "visual.jpg".to_owned(),
            size: 1024,
            width: 640,
            height: 480,
            kind: VisualKind::Image,
            duration_ms: 0,
            hash: "hash".to_owned(),
            file_id: "file-id".to_owned(),
            comment_count: 0,
            like_count: 0,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    fn assert_token(token: &str, cipher: &TokenCipher, viewer: UserId, expected: VisualTokenKind) {
        let token = cipher.decrypt::<VisualToken>(token).unwrap();
        assert_eq!(token.file_id, "file-id");
        assert_eq!(token.viewer_id, viewer);
        assert_eq!(token.kind, expected);
    }

    #[test]
    fn from_record_with_tokens_preserves_fields_and_generates_tokens() {
        let viewer = UserId(3);
        let cipher = test_cipher();
        let record = visual_record();
        let expected_created_at = record.created_at;

        let view = VisualView::from_record_with_tokens(record, viewer).unwrap();

        assert_eq!(view.id, VisualId(1));
        assert_eq!(view.user_id, UserId(2));
        assert_eq!(view.name, "visual.jpg");
        assert_eq!(view.created_at, expected_created_at);
        assert_token(
            &view.tokens.thumbnail,
            cipher,
            viewer,
            VisualTokenKind::Image(ImageTokenType::Thumbnail),
        );
        assert_token(
            &view.tokens.preview,
            cipher,
            viewer,
            VisualTokenKind::Image(ImageTokenType::Preview),
        );
        assert_token(
            &view.tokens.original,
            cipher,
            viewer,
            VisualTokenKind::Image(ImageTokenType::Original),
        );
    }

    #[test]
    fn from_video_record_generates_video_tokens() {
        let viewer = UserId(3);
        let cipher = test_cipher();
        let record = VisualRecord {
            kind: VisualKind::Video,
            duration_ms: 10_000,
            ..visual_record()
        };

        let view = VisualView::from_record_with_tokens(record, viewer).unwrap();

        assert_token(
            &view.tokens.thumbnail,
            cipher,
            viewer,
            VisualTokenKind::Video(VideoTokenType::Thumbnail),
        );
        assert_token(
            &view.tokens.preview,
            cipher,
            viewer,
            VisualTokenKind::Video(VideoTokenType::Preview),
        );
        assert_token(
            &view.tokens.original,
            cipher,
            viewer,
            VisualTokenKind::Video(VideoTokenType::Original),
        );
    }
}

impl Default for VisualCursorParam {
    /// 默认游标参数:无游标,每页 32 条,方向为 Next
    fn default() -> Self {
        Self {
            cursor: None,
            size: 32,
            direction: PageDirection::Next,
            anchor_time: None,
        }
    }
}

/// 分页方向定义在共享内核 `types-core`(跨上下文通用词汇),此处重导出以保持
/// `types_visual::PageDirection` 路径不变。
pub use types_core::PageDirection;
