use validator::Validate;

use crate::VisualTokenStr;
use crate::models::PersonName;
use crate::person::PersonId;
#[cfg(feature = "face")]
use crate::person::PersonRecord;
use crate::visual::VisualId;
#[cfg(feature = "face")]
use crate::{FaceBBox, ImageDimensions, VisualToken};
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
        // 人脸框原始宽高各异。给人物卡片封面生成统一的正方形裁切,
        // 并在脸框周围留出固定比例的背景,避免列表中人脸大小忽大忽小。
        let bbox = square_person_cover_bbox(value.cover.bbox, dimension);
        let cover_token = VisualTokenStr::encrypt(&VisualToken::crop(
            viewer,
            value.cover.file_id,
            bbox,
            dimension,
        ))?;

        Ok(Self {
            id: value.id,
            name: value.name,
            cover_token,
            face_count: value.face_count,
        })
    }
}

/// 返回以人脸框中心为中心、边长为人脸较长边 1.5 倍的正方形封面裁剪框。
/// 靠近原图边缘时平移裁剪框以尽可能保留完整的人脸。
#[cfg(feature = "face")]
fn square_person_cover_bbox(bbox: FaceBBox, dimensions: ImageDimensions) -> FaceBBox {
    let image_width = dimensions.width as i32;
    let image_height = dimensions.height as i32;
    if image_width <= 0 || image_height <= 0 {
        return bbox;
    }

    let (x, y, width, height) = bbox.to_pixel_rect(dimensions.width, dimensions.height);
    if width <= 0 || height <= 0 {
        return bbox;
    }

    let face_side = width.max(height);
    let max_square_side = image_width.min(image_height);
    let square_side = ((face_side as f32 * 1.5).round() as i32).min(max_square_side);
    // 原图本身不足以容纳完整的人脸正方形时,保留原始框避免裁掉人脸。
    if square_side < face_side {
        return bbox;
    }

    let center_x = x + width / 2;
    let center_y = y + height / 2;
    let left = (center_x - square_side / 2).clamp(0, image_width - square_side);
    let top = (center_y - square_side / 2).clamp(0, image_height - square_side);
    let right = left + square_side;
    let bottom = top + square_side;

    FaceBBox {
        x1: left as f32 / image_width as f32,
        y1: top as f32 / image_height as f32,
        x2: right as f32 / image_width as f32,
        y2: bottom as f32 / image_height as f32,
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

    #[cfg(feature = "face")]
    #[test]
    fn person_cover_crop_is_square_with_consistent_padding() {
        let bbox = FaceBBox {
            x1: 0.4,
            y1: 0.4,
            x2: 0.5,
            y2: 0.6,
        };
        let dimensions = ImageDimensions {
            width: 1000,
            height: 800,
        };

        let cover = square_person_cover_bbox(bbox, dimensions);
        let (_, _, width, height) = cover.to_pixel_rect(dimensions.width, dimensions.height);

        assert_eq!(width, 240);
        assert_eq!(height, 240);
    }

    #[cfg(feature = "face")]
    #[test]
    fn person_cover_crop_shifts_inward_near_image_edges() {
        let bbox = FaceBBox {
            x1: 0.01,
            y1: 0.01,
            x2: 0.11,
            y2: 0.16,
        };
        let dimensions = ImageDimensions {
            width: 1000,
            height: 800,
        };

        let cover = square_person_cover_bbox(bbox, dimensions);
        let (x, y, width, height) = cover.to_pixel_rect(dimensions.width, dimensions.height);

        assert_eq!((x, y), (0, 0));
        assert_eq!(width, height);
        assert!(cover.x2 >= bbox.x2);
        assert!(cover.y2 >= bbox.y2);
    }

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
