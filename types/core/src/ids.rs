//! 强类型 ID：每个限界上下文的主键类型集中声明于此。
//!
//! ID 是跨上下文交互的通用语言(外键列、DTO 字段、游标),因此属于共享内核,
//! 由各域契约 crate 重新导出以保持既有路径
//! (如 `types::visual::visual::VisualId`、`types::auth::user::UserId`)。

// ── 用户身份 ──
crate::id_type!(UserId, "user/");

// ── 审计 ──
crate::id_type!(AuditId, "audit/");

// ── 视觉 ──
crate::id_type!(VisualId, "visual/");
crate::id_type!(CollectionId, "visual/");
crate::id_type!(CollectionVisualId, "visual/");
crate::id_type!(CommentId, "visual/");
crate::id_type!(CommentLikeId, "visual/");
crate::id_type!(VisualLikeId, "visual/");
crate::id_type!(TimelineStatId, String, "visual/");
crate::id_type!(FaceId, "visual/");
crate::id_type!(PersonId, "visual/");

/// 已通过管理员校验的用户身份。
///
/// 被身份(认证 / 资料)、审计、备份与视觉上下文共同引用, 因此与 ID 一同放在共享内核。
/// 由 [`AdminId::new`] 构造,只有管理员才能取得;作为 service 层参数,
/// 内部通过 [`AdminId::into_inner`] 展开为 [`UserId`] 使用。
#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::Display)]
#[display("{}", _0)]
pub struct AdminId(UserId);

impl AdminId {
    pub const ADMIN_ID: AdminId = AdminId(UserId(1));

    /// 判断该身份是否为系统管理员.
    pub fn is_admin(&self) -> bool {
        *self == Self::ADMIN_ID
    }

    /// 展开为内部 [`UserId`]
    pub fn into_inner(self) -> UserId {
        self.0
    }

    /// 校验管理员权限，非管理员返回 403
    #[cfg(feature = "orm")]
    pub fn new(user_id: UserId) -> common_core::Result<Self> {
        let this = Self(user_id);
        if this.is_admin() {
            Ok(this)
        } else {
            Err(common_core::error::AppError::forbidden("仅管理员可访问"))
        }
    }
}
