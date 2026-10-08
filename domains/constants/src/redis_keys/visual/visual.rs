use types_core::{PageDirection, UserId, VisualId};

/// 生成影像信息的 Redis 缓存键；缓存不含按浏览者签发的 token，同一影像共享一份缓存
#[inline]
pub fn visual_info(visual_id: VisualId) -> String {
    //visual:visual:info
    format!("v:v:i:{}", visual_id)
}

/// 生成影像尺寸缓存的 Redis 缓存键；按 file_id 拆分
#[inline]
pub fn visual_dimensions(file_id: &str) -> String {
    //visual:visual:dimensions
    format!("v:v:d:{}", file_id)
}

/// 生成用户对影像点赞状态的 Redis 缓存键；键同时包含用户与影像 ID
#[inline]
pub fn visual_is_liked(user_id: UserId, visual_id: VisualId) -> String {
    //visual:visual:is_liked
    format!("v:v:l:{}:{}", user_id, visual_id)
}

/// 生成影像首屏 ID 列表的 Redis 缓存键；缓存每个方向的最大首屏，调用方按实际页大小截断
#[inline]
pub fn visual_cursor_page_ids(direction: PageDirection) -> &'static str {
    //visual:visual:cursor_page_ids
    match direction {
        PageDirection::Next => "v:v:c:n",
        PageDirection::Prev => "v:v:c:p",
    }
}

/// 生成影像处理产物的 Redis 缓存键；键包含原图 file_id 与处理方式 `op_key`
#[inline]
pub fn visual_processed(file_id: &str, op_key: &str) -> String {
    //visual:visual:processed
    format!("v:v:p:{op_key}:{file_id}")
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

    #[test]
    fn visual_processed_returns_correct_format() {
        let key = visual_processed("visuals/2024/01/01/abc.jpg", "local:thumb");
        assert_eq!(key, "v:v:p:local:thumb:visuals/2024/01/01/abc.jpg");
    }
}
