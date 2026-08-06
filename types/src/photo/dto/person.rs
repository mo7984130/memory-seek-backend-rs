use crate::cursor::TimeIdCursor;
use crate::photo::models::PersonName;
use crate::photo::person::PersonId;
use crate::photo::photo::PhotoId;

crate::in_dto!(PersonCursorParam, "photo/", serde_default; {
    pub cursor: Option<PersonId>,
    pub size: u64,
});

impl Default for PersonCursorParam {
    fn default() -> Self {
        Self {
            cursor: None,
            size: 32,
        }
    }
}

crate::in_dto!(PersonSearchParam, "photo/", serde_default, docs = "人物搜索参数(keyword 前缀匹配完整名字或姓名首字母)"; {
    /// 搜索关键词: 完整名字或姓名首字母(如 张三 / ZS)
    #[validate(length(min = 1, max = 64, message = "搜索关键词长度在 1 到 64 之间"))]
    pub keyword: String,
    pub cursor: Option<PersonId>,
    pub size: u64,
});

impl Default for PersonSearchParam {
    fn default() -> Self {
        Self {
            keyword: String::new(),
            cursor: None,
            size: 32,
        }
    }
}

crate::out_dto!(PersonView, "photo/", rename = "Person"; {
    pub id: PersonId,
    pub name: String,
    /// 封面图 token(加密串, 经 `GET /photo/image/{token}` 访问)
    pub cover_token: Option<String>,
    pub face_count: u64
});

pub const PERSON_PHOTO_CURSOR_PAGE_DEFAULT_SIZE: u64 = 32;

fn person_photo_cursor_page_default_size() -> u64 {
    PERSON_PHOTO_CURSOR_PAGE_DEFAULT_SIZE
}

crate::in_dto!(PersonPhotoCursorParam, "photo/", docs = "人物照片游标参数(cursor 为 TimeIdCursor<PhotoId> 的 Base64 编码)"; {
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub cursor: Option<TimeIdCursor<PhotoId>>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[serde(default = "person_photo_cursor_page_default_size")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: u64,
});

crate::in_dto!(RenamePersonParam, "photo/", docs = "重命名人物参数"; {
    pub new_name: PersonName,
});

fn secondary_cluster_default_threshold() -> f32 {
    0.55
}

crate::in_dto!(SecondaryClusterParam, "photo/", docs = "二次聚类参数(将未分配人脸按 centroid 余弦相似度指派到已有人物)"; {
    /// 余弦相似度阈值: 高于等于该值才指派, 否则保持未分配(范围 0 到 1, 默认 0.55)
    #[validate(range(min = 0.0, max = 1.0, message = "相似度阈值在 0 到 1 之间"))]
    #[serde(default = "secondary_cluster_default_threshold")]
    pub threshold: f32,
});

/// 合并人物参数
#[derive(Debug, serde::Deserialize, validator::Validate)]
#[serde(rename_all = "camelCase", try_from = "MergePersonParamInner")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "photo/"))]
pub struct MergePersonParam {
    pub source_person_id: PersonId,
    pub target_person_id: PersonId,
}

/// 反序列化中间类型: 反序列化后经 `TryFrom` 校验跨字段关系
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergePersonParamInner {
    source_person_id: PersonId,
    target_person_id: PersonId,
}

impl TryFrom<MergePersonParamInner> for MergePersonParam {
    type Error = String;

    fn try_from(inner: MergePersonParamInner) -> Result<Self, Self::Error> {
        if inner.source_person_id == inner.target_person_id {
            return Err("不能将人物合并到自身".to_string());
        }
        Ok(Self {
            source_person_id: inner.source_person_id,
            target_person_id: inner.target_person_id,
        })
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
        let result = serde_json::from_str::<MergePersonParam>(json);
        assert!(result.is_err());
    }
}
