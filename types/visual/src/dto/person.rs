use validator::Validate;

use crate::VisualTokenStr;
use crate::models::PersonName;
use crate::person::PersonId;
#[cfg(feature = "face")]
use crate::person::PersonRecord;
use crate::visual::VisualId;
#[cfg(feature = "face")]
use crate::{ImageDimensions, VisualToken};
#[cfg(feature = "face")]
use types_core::UserId;
use types_core::cursor::{CountIdCursor, TimeIdCursor};

crate::dto!(PersonCursorParam, "visual/", serde_default, docs = "人物列表参数(cursor 为 FaceCountIdCursor<PersonId> 的 Base64 编码, 按 face_count 倒序分页)"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<CountIdCursor<PersonId>>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

impl Default for PersonCursorParam {
    /// 默认分页参数:无游标,每页 32 条
    fn default() -> Self {
        Self {
            cursor: None,
            size: 32,
        }
    }
}

crate::dto!(PersonSearchParam, "visual/", serde_default, docs = "人物搜索参数(keyword 前缀匹配完整名字或姓名首字母)"; {
    /// 搜索关键词: 完整名字或姓名首字母(如 张三 / ZS)
    #[validate(length(min = 1, max = 64, message = "搜索关键词长度在 1 到 64 之间"))]
    pub keyword: String,
    pub cursor: Option<PersonId>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

impl Default for PersonSearchParam {
    /// 默认搜索参数:空关键词,每页 32 条
    fn default() -> Self {
        Self {
            keyword: String::new(),
            cursor: None,
            size: 32,
        }
    }
}

crate::dto!(PersonView, "visual/", rename = "Person"; {
    pub id: PersonId,
    pub name: String,
    /// 封面图 token(加密串, 经 `GET /visual/{token}` 访问)
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub cover_token: VisualTokenStr,
    pub face_count: u64
});
#[cfg(feature = "face")]
impl PersonView {
    /// 由人物记录、查看者与图片尺寸构造人物视图,并生成封面裁剪令牌
    pub fn from_record(
        value: PersonRecord,
        viewer: UserId,
        dimension: ImageDimensions,
    ) -> common_core::ContextualResult<Self> {
        Ok(Self {
            id: value.id,
            name: value.name,
            cover_token: VisualTokenStr::encrypt(&VisualToken::crop(
                viewer,
                value.cover.file_id,
                value.cover.bbox,
                dimension,
            ))?,
            face_count: value.face_count,
        })
    }
}

pub const PERSON_PHOTO_CURSOR_PAGE_DEFAULT_SIZE: u64 = 32;

/// 返回人物照片分页的默认页大小
fn person_visual_cursor_page_default_size() -> u64 {
    PERSON_PHOTO_CURSOR_PAGE_DEFAULT_SIZE
}

crate::dto!(PersonVisualCursorParam, "visual/", docs = "人物照片游标参数(cursor 为 TimeIdCursor<VisualId> 的 Base64 编码)"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<VisualId>>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[serde(default = "person_visual_cursor_page_default_size")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub size: u64,
});

crate::dto!(RenamePersonParam, "visual/", docs = "重命名人物参数"; {
    #[validate(nested)]
    pub new_name: PersonName,
});

/// 返回二次人物聚类的默认相似度阈值
fn secondary_cluster_default_threshold() -> f32 {
    0.55
}

crate::dto!(SecondaryClusterParam, "visual/", docs = "二次聚类参数(将未分配人脸按 centroid 余弦相似度指派到已有人物)"; {
    /// 余弦相似度阈值: 高于等于该值才指派, 否则保持未分配(范围 0 到 1, 默认 0.55)
    #[validate(range(min = 0.0, max = 1.0, message = "相似度阈值在 0 到 1 之间"))]
    #[serde(default = "secondary_cluster_default_threshold")]
    pub threshold: f32,
});

/// 合并人物参数
///
/// 跨字段校验("不能合并到自身")放在 `Validate` 阶段,而非 `try_from`
/// 反序列化校验,避免业务错误被 serde 误判为解析错误并混入位置信息。
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "visual/"))]
pub struct MergePersonParam {
    pub source_person_id: PersonId,
    pub target_person_id: PersonId,
}

impl validator::Validate for MergePersonParam {
    /// 校验源与目标人物不是同一人
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        if self.source_person_id == self.target_person_id {
            let mut err = validator::ValidationError::new("self_merge");
            err.message = Some(std::borrow::Cow::Borrowed("不能将人物合并到自身"));
            let mut errors = validator::ValidationErrors::new();
            errors.add("source_person_id", err);
            return Err(errors);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_merge_person_param_deserialize_valid() {
        let json = r#"{"sourcePersonId": 1, "targetPersonId": 2}"#;
        let param: MergePersonParam = serde_json::from_str(json).unwrap();
        assert_eq!(param.source_person_id, PersonId(1));
        assert_eq!(param.target_person_id, PersonId(2));
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_merge_person_param_deserialize_self_merge_rejected() {
        let json = r#"{"sourcePersonId": 1, "targetPersonId": 1}"#;
        let param: MergePersonParam = serde_json::from_str(json).unwrap();
        assert!(param.validate().is_err());
    }
}
