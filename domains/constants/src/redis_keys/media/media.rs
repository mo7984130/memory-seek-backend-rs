use types::{
    auth::user::UserId,
    media::{dto::media::PageDirection, media::MediaId},
};

/// 生成媒体信息的 Redis 缓存键
///
/// 缓存内容为 `MediaRecord`，不含按浏览者签发的 token（token 在读取时按浏览者
/// 动态生成），因此键只按媒体拆分，同一媒体所有浏览者共享一份缓存。
///
/// # 参数
/// - `media_id`: 媒体 ID
///
/// # 返回
/// 格式为 `p:p:i:{media_id}` 的缓存键
#[inline]
pub fn media_info(media_id: MediaId) -> String {
    //media:media:info
    format!("m:m:i:{}", media_id)
}

/// 生成媒体尺寸缓存的 Redis 缓存键
///
/// 缓存内容为 `(width, height)` 元组，按 file_id 拆分（尺寸是媒体的静态元数据）。
///
/// # 参数
/// - `file_id`: 媒体文件 ID
///
/// # 返回
/// 格式为 `p:p:d:{file_id}` 的缓存键
#[inline]
pub fn media_dimensions(file_id: &str) -> String {
    //media:media:dimensions
    format!("m:m:d:{}", file_id)
}

/// 生成用户对媒体点赞状态的 Redis 缓存键。
///
/// 点赞状态取决于浏览者，因此键同时包含用户和媒体 ID。
///
/// # 返回
/// 格式为 `p:p:l:{user_id}:{media_id}` 的缓存键
#[inline]
pub fn media_is_liked(user_id: UserId, media_id: MediaId) -> String {
    //media:media:is_liked
    format!("m:m:l:{}:{}", user_id, media_id)
}

/// 生成媒体首屏 ID 列表的 Redis 缓存键。
///
/// 缓存存储每个方向允许的最大首屏，调用方再按实际页大小截断，因此键无需包含
/// `size` 且能被上传、删除操作准确失效。
#[inline]
pub fn media_cursor_page_ids(direction: PageDirection) -> &'static str {
    //media:media:cursor_page_ids
    match direction {
        PageDirection::Next => "m:m:c:n",
        PageDirection::Prev => "m:m:c:p",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_info_returns_correct_format() {
        let key = media_info(MediaId(42));
        assert_eq!(key, "m:m:i:42");
    }

    #[test]
    fn media_info_different_ids_produce_different_keys() {
        let key1 = media_info(MediaId(1));
        let key2 = media_info(MediaId(2));
        assert_ne!(key1, key2);
    }

    #[test]
    fn media_dimensions_returns_correct_format() {
        let key = media_dimensions("medias/2024/01/01/abc.jpg");
        assert_eq!(key, "m:m:d:medias/2024/01/01/abc.jpg");
    }

    #[test]
    fn media_is_liked_returns_correct_format() {
        let key = media_is_liked(UserId(7), MediaId(42));
        assert_eq!(key, "m:m:l:7:42");
    }

    #[test]
    fn media_cursor_page_ids_returns_correct_format() {
        assert_eq!(media_cursor_page_ids(PageDirection::Next), "m:m:c:n");
        assert_eq!(media_cursor_page_ids(PageDirection::Prev), "m:m:c:p");
    }
}
