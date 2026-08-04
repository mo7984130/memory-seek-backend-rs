use crate::photo::person::PersonId;

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
    pub cursor: Option<String>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    #[serde(default = "person_photo_cursor_page_default_size")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: u64,
});

crate::in_dto!(RenamePersonParam, "photo/", docs = "重命名人物参数"; {
    #[validate(length(min = 1, max = 64, message = "人物名称长度在 1 到 64 之间"))]
    pub new_name: String,
});

crate::in_dto!(MergePersonParam, "photo/", docs = "合并人物参数"; {
    pub source_person_id: PersonId,
    pub target_person_id: PersonId,
});
