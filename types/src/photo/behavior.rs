// ============================================================
// UserBehaviorId
// ============================================================

crate::id_type!(UserBehaviorId, "photo/");

// ============================================================
// 行为枚举（始终可用，供 DTO/TS 导出）
// ============================================================

use serde::{Deserialize, Serialize};

use crate::error::ParseEnumError;

/// 用户行为动作（审计用，只追加不删除）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "photo/"))]
pub enum UserBehaviorAction {
    /// 浏览照片（仅预览/原图访问时记录）
    View,
    /// 上传照片
    Upload,
    /// 批量删除照片
    DeletePhotos,
    /// 点赞照片
    Like,
    /// 取消点赞照片
    Unlike,
    /// 发布评论
    CommentPublish,
    /// 删除评论
    CommentDelete,
    /// 点赞评论
    CommentLike,
    /// 取消点赞评论
    CommentUnlike,
    /// 收藏照片
    Collect,
    /// 取消收藏照片
    Uncollect,
    /// 修改人脸归属
    FaceChangeBelonging,
    /// 取消人脸归属
    FaceUnassign,
    /// 删除人脸
    FaceDelete,
    /// 人脸计算任务（全量/增量）
    FaceCompute,
    /// 重命名人物
    PersonRename,
    /// 合并人物
    PersonMerge,
    /// 删除人物
    PersonDelete,
    /// 人物全量扫描
    PersonFullScan,
    /// 人物二次聚类
    PersonSecondaryCluster,
}

impl UserBehaviorAction {
    /// 数据库存储的字符串值
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Upload => "upload",
            Self::DeletePhotos => "delete_photos",
            Self::Like => "like",
            Self::Unlike => "unlike",
            Self::CommentPublish => "comment_publish",
            Self::CommentDelete => "comment_delete",
            Self::CommentLike => "comment_like",
            Self::CommentUnlike => "comment_unlike",
            Self::Collect => "collect",
            Self::Uncollect => "uncollect",
            Self::FaceChangeBelonging => "face_change_belonging",
            Self::FaceUnassign => "face_unassign",
            Self::FaceDelete => "face_delete",
            Self::FaceCompute => "face_compute",
            Self::PersonRename => "person_rename",
            Self::PersonMerge => "person_merge",
            Self::PersonDelete => "person_delete",
            Self::PersonFullScan => "person_full_scan",
            Self::PersonSecondaryCluster => "person_secondary_cluster",
        }
    }
}

impl std::str::FromStr for UserBehaviorAction {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "view" => Ok(Self::View),
            "upload" => Ok(Self::Upload),
            "delete_photos" => Ok(Self::DeletePhotos),
            "like" => Ok(Self::Like),
            "unlike" => Ok(Self::Unlike),
            "comment_publish" => Ok(Self::CommentPublish),
            "comment_delete" => Ok(Self::CommentDelete),
            "comment_like" => Ok(Self::CommentLike),
            "comment_unlike" => Ok(Self::CommentUnlike),
            "collect" => Ok(Self::Collect),
            "uncollect" => Ok(Self::Uncollect),
            "face_change_belonging" => Ok(Self::FaceChangeBelonging),
            "face_unassign" => Ok(Self::FaceUnassign),
            "face_delete" => Ok(Self::FaceDelete),
            "face_compute" => Ok(Self::FaceCompute),
            "person_rename" => Ok(Self::PersonRename),
            "person_merge" => Ok(Self::PersonMerge),
            "person_delete" => Ok(Self::PersonDelete),
            "person_full_scan" => Ok(Self::PersonFullScan),
            "person_secondary_cluster" => Ok(Self::PersonSecondaryCluster),
            _ => Err(ParseEnumError(s.to_string())),
        }
    }
}

/// 行为目标类型
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "photo/"))]
pub enum BehaviorTargetType {
    Photo,
    Face,
    Person,
    Comment,
    Collection,
}

impl BehaviorTargetType {
    /// 数据库存储的字符串值
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Photo => "photo",
            Self::Face => "face",
            Self::Person => "person",
            Self::Comment => "comment",
            Self::Collection => "collection",
        }
    }
}

impl std::str::FromStr for BehaviorTargetType {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "photo" => Ok(Self::Photo),
            "face" => Ok(Self::Face),
            "person" => Ok(Self::Person),
            "comment" => Ok(Self::Comment),
            "collection" => Ok(Self::Collection),
            _ => Err(ParseEnumError(s.to_string())),
        }
    }
}

// ============================================================
// SeaORM 实体（仅 orm feature）
// ============================================================

#[cfg(feature = "orm")]
mod entity {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::auth::user::UserId;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "photo_user_behavior")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: UserBehaviorId,
        pub user_id: UserId,
        pub action: String,
        pub target_type: Option<String>,
        pub target_id: Option<i64>,
        #[sea_orm(column_type = "Json")]
        pub detail: Option<Json>,
        pub ip: Option<String>,
        pub created_at: DateTimeUtc,
    }

    /// 用户行为记录，使用强类型枚举
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct BehaviorRecord {
        pub id: UserBehaviorId,
        pub user_id: UserId,
        pub action: UserBehaviorAction,
        pub target_type: Option<BehaviorTargetType>,
        pub target_id: Option<i64>,
        pub detail: Option<Json>,
        pub ip: Option<String>,
        pub created_at: DateTimeUtc,
    }

    impl From<Model> for BehaviorRecord {
        fn from(model: Model) -> Self {
            Self {
                id: model.id,
                user_id: model.user_id,
                action: model.action.parse().unwrap_or(UserBehaviorAction::View),
                target_type: model.target_type.as_deref().and_then(|s| s.parse().ok()),
                target_id: model.target_id,
                detail: model.detail,
                ip: model.ip,
                created_at: model.created_at,
            }
        }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "orm")]
pub use entity::*;
