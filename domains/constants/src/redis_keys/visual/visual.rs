use types_core::{PageDirection, UserId, VisualId};

/// 生成影像信息的 Redis 缓存键
///
/// 缓存内容为 `VisualRecord`，不含按浏览者签发的 token（token 在读取时按浏览者
/// 动态生成），因此键只按影像拆分，同一影像所有浏览者共享一份缓存。
///
/// # 参数
/// - `visual_id`: 影像 ID
///
/// # 返回
/// 格式为 `p:p:i:{visual_id}` 的缓存键
#[inline]
pub fn visual_info(visual_id: VisualId) -> String {
    //visual:visual:info
    format!("v:v:i:{}", visual_id)
}

/// 生成影像尺寸缓存的 Redis 缓存键
///
/// 缓存内容为 `(width, height)` 元组，按 file_id 拆分（尺寸是影像的静态元数据）。
///
/// # 参数
/// - `file_id`: 影像文件 ID
///
/// # 返回
/// 格式为 `p:p:d:{file_id}` 的缓存键
#[inline]
pub fn visual_dimensions(file_id: &str) -> String {
    //visual:visual:dimensions
    format!("v:v:d:{}", file_id)
}

/// 生成用户对影像点赞状态的 Redis 缓存键。
///
/// 点赞状态取决于浏览者，因此键同时包含用户和影像 ID。
///
/// # 返回
/// 格式为 `p:p:l:{user_id}:{visual_id}` 的缓存键
#[inline]
pub fn visual_is_liked(user_id: UserId, visual_id: VisualId) -> String {
    //visual:visual:is_liked
    format!("v:v:l:{}:{}", user_id, visual_id)
}

/// 生成影像首屏 ID 列表的 Redis 缓存键。
///
/// 缓存存储每个方向允许的最大首屏，调用方再按实际页大小截断，因此键无需包含
/// `size` 且能被上传、删除操作准确失效。
#[inline]
pub fn visual_cursor_page_ids(direction: PageDirection) -> &'static str {
    //visual:visual:cursor_page_ids
    match direction {
        PageDirection::Next => "v:v:c:n",
        PageDirection::Prev => "v:v:c:p",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_info_returns_correct_format() {
        let key = visual_info(VisualId(42));
        assert_eq!(key, "v:v:i:42");
    }

    #[test]
    fn visual_info_different_ids_produce_different_keys() {
        let key1 = visual_info(VisualId(1));
        let key2 = visual_info(VisualId(2));
        assert_ne!(key1, key2);
    }

    #[test]
    fn visual_dimensions_returns_correct_format() {
        let key = visual_dimensions("visuals/2024/01/01/abc.jpg");
        assert_eq!(key, "v:v:d:visuals/2024/01/01/abc.jpg");
    }

    #[test]
    fn visual_is_liked_returns_correct_format() {
        let key = visual_is_liked(UserId(7), VisualId(42));
        assert_eq!(key, "v:v:l:7:42");
    }

    #[test]
    fn visual_cursor_page_ids_returns_correct_format() {
        assert_eq!(visual_cursor_page_ids(PageDirection::Next), "v:v:c:n");
        assert_eq!(visual_cursor_page_ids(PageDirection::Prev), "v:v:c:p");
    }
}
