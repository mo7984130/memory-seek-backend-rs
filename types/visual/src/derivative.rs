// ============================================================
// VisualDerivativeId
// ============================================================

pub use types_core::VisualDerivativeId;

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use common_core::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::visual::VisualId;

    /// 视频衍生片类型: 缩略预览片 / 完整压缩预览片
    #[derive(
        Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
    )]
    #[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
    pub enum DerivativeKind {
        /// 前若干秒的缩略预览片
        #[sea_orm(string_value = "thumbnail")]
        Thumbnail,
        /// 整片压缩预览片
        #[sea_orm(string_value = "preview")]
        Preview,
    }

    impl DerivativeKind {
        /// 衍生对象在对象存储中的文件名(不含目录)
        pub fn file_name(self) -> &'static str {
            match self {
                Self::Thumbnail => "thumb.mp4",
                Self::Preview => "preview.mp4",
            }
        }

        /// 衍生对象 MIME 类型
        pub fn mime_type(self) -> &'static str {
            "video/mp4"
        }
    }

    /// 衍生片生成状态
    #[derive(
        Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
    )]
    #[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
    pub enum DerivativeStatus {
        /// 待生成
        #[sea_orm(string_value = "pending")]
        Pending,
        /// 生成中
        #[sea_orm(string_value = "running")]
        Running,
        /// 已就绪
        #[sea_orm(string_value = "ready")]
        Ready,
        /// 生成失败
        #[sea_orm(string_value = "failed")]
        Failed,
    }

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "visual_derivative")]
    pub struct Model {
        /// 主键ID
        #[sea_orm(primary_key)]
        pub id: VisualDerivativeId,

        /// 所属影像ID
        #[sea_orm(indexed)]
        pub visual_id: VisualId,

        /// 衍生片类型
        pub kind: DerivativeKind,

        /// 生成状态
        pub status: DerivativeStatus,

        /// 生成成功后的对象 key(失败/未就绪时为空)
        pub object_key: Option<String>,

        /// 最近一次失败原因
        pub error: Option<String>,

        /// 已尝试次数(含首次); 用于有限重试
        #[sea_orm(default_value = 0)]
        pub attempts: i32,

        /// 更新时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub updated_at: DateTime,

        /// 创建时间
        #[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]
        pub created_at: DateTime,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}

    /// 衍生片记录(强类型)
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct DerivativeRecord {
        pub id: VisualDerivativeId,
        pub visual_id: VisualId,
        pub kind: DerivativeKind,
        pub status: DerivativeStatus,
        pub object_key: Option<String>,
        pub error: Option<String>,
        pub attempts: i32,
    }

    impl From<Model> for DerivativeRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                visual_id: model.visual_id,
                kind: model.kind,
                status: model.status,
                object_key: model.object_key,
                error: model.error,
                attempts: model.attempts,
            }
        }
    }

    /// 待持久化的衍生片记录
    pub struct NewDerivativeRecord {
        pub visual_id: VisualId,
        pub kind: DerivativeKind,
    }

    impl From<NewDerivativeRecord> for ActiveModel {
        fn from(record: NewDerivativeRecord) -> Self {
            use sea_orm::ActiveValue::Set;
            Self {
                visual_id: Set(record.visual_id),
                kind: Set(record.kind),
                status: Set(DerivativeStatus::Pending),
                ..Default::default()
            }
        }
    }

    /// 依原影像对象 key 推导衍生片对象 key.
    ///
    /// 原 key 形如 `visuals/{y}/{m}/{d}/{uuid}.{ext}`, 衍生 key 为
    /// `visuals/{y}/{m}/{d}/{uuid}/{file_name}`。
    pub fn derivative_key(file_id: &str, kind: DerivativeKind) -> String {
        let base = file_id
            .rsplit_once('.')
            .map(|(base, _)| base)
            .unwrap_or(file_id);
        format!("{}/{}", base.trim_end_matches('/'), kind.file_name())
    }
}

#[cfg(feature = "orm")]
pub use entity::*;

#[cfg(all(test, feature = "orm"))]
mod tests {
    use super::*;

    #[test]
    fn derivative_key_uses_dir_under_original() {
        assert_eq!(
            derivative_key("visuals/2026/09/28/abc.mp4", DerivativeKind::Thumbnail),
            "visuals/2026/09/28/abc/thumb.mp4"
        );
        assert_eq!(
            derivative_key("visuals/2026/09/28/abc.mov", DerivativeKind::Preview),
            "visuals/2026/09/28/abc/preview.mp4"
        );
    }
}
