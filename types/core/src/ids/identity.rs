//! 用户身份上下文的主键(`auth_user` 表)。
//!
//! `UserId` 被身份、视觉(`visual.user_id` 等外键)、审计(`actor_id`)、
//! 令牌(`VisualToken.viewer_id`)与备份共同引用,因此位于共享内核。

crate::id_type!(UserId, "user/");

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
