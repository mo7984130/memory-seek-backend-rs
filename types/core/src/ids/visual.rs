//! 视觉上下文的主键:影像、相册、评论与人脸等实体。
//!
//! 这些表同属视觉限界上下文(`types_visual`),ID 集中在此是因为视觉实体与 DTO
//! 分别由 `types-visual` 与视觉域使用,且视觉 ID 的线格式需与其它上下文一致。

crate::id_type!(VisualId, "visual/");
crate::id_type!(CollectionId, "visual/");
crate::id_type!(CollectionVisualId, "visual/");
crate::id_type!(CommentId, "visual/");
crate::id_type!(CommentLikeId, "visual/");
crate::id_type!(VisualLikeId, "visual/");
crate::id_type!(TimelineStatId, String, "visual/");
crate::id_type!(FaceId, "visual/");
crate::id_type!(PersonId, "visual/");
