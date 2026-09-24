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
