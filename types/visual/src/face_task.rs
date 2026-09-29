// ============================================================
// SeaORM 实体（仅 face-engine feature）
// ============================================================
//
// 人脸检测任务的持久化 outbox: 每张图片一条, 承载"上传后待人脸检测"的进度。
// 内存队列(mpsc)只做唤醒, 崩溃后由本表在启动时恢复未完成任务(见 FaceService)。

#[cfg(feature = "face-engine")]
mod entity {
    use common_core::time::DateTime;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use crate::visual::VisualId;

    /// 人脸检测任务状态
    #[derive(
        Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
    )]
    #[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
    pub enum FaceTaskStatus {
        /// 待检测
        #[sea_orm(string_value = "pending")]
        Pending,
        /// 检测中
        #[sea_orm(string_value = "running")]
        Running,
        /// 检测完成(已落库, 含"零人脸"结论)
        #[sea_orm(string_value = "done")]
        Done,
        /// 检测失败
        #[sea_orm(string_value = "failed")]
        Failed,
    }

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "visual_face_task")]
    pub struct Model {
        /// 主键: 影像ID(每张影像至多一条人脸检测任务)
        #[sea_orm(primary_key, auto_increment = false)]
        pub visual_id: VisualId,

        /// 检测状态
        pub status: FaceTaskStatus,

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

    /// 人脸检测任务记录(强类型)
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct FaceTaskRecord {
        pub visual_id: VisualId,
        pub status: FaceTaskStatus,
        pub error: Option<String>,
        pub attempts: i32,
    }

    impl From<Model> for FaceTaskRecord {
        fn from(model: Model) -> Self {
            Self {
                visual_id: model.visual_id,
                status: model.status,
                error: model.error,
                attempts: model.attempts,
            }
        }
    }

    /// 待持久化的人脸检测任务
    pub struct NewFaceTaskRecord {
        pub visual_id: VisualId,
    }

    impl From<NewFaceTaskRecord> for ActiveModel {
        fn from(record: NewFaceTaskRecord) -> Self {
            use sea_orm::ActiveValue::Set;
            Self {
                visual_id: Set(record.visual_id),
                status: Set(FaceTaskStatus::Pending),
                ..Default::default()
            }
        }
    }
}

#[cfg(feature = "face-engine")]
pub use entity::*;
