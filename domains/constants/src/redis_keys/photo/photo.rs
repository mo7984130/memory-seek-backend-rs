use types::auth::user::UserId;
use types::photo::photo::PhotoId;

/// 生成照片信息的 Redis 缓存键
///
/// 缓存内容包含按浏览者签发的图片 token，因此键按用户拆分，避免 token 串用。
///
/// # 参数
/// - `photo_id`: 照片 ID
/// - `user_id`: 浏览者（token 签发给谁）
///
/// # 返回
/// 格式为 `p:i:{user_id}:{photo_id}` 的缓存键
#[inline]
pub fn photo_info(photo_id: PhotoId, user_id: UserId) -> String {
    //photo:photo:info
    format!("p:p:i:{}:{}", user_id, photo_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_info_returns_correct_format() {
        let key = photo_info(PhotoId(42), UserId(7));
        assert_eq!(key, "p:p:i:7:42");
    }

    #[test]
    fn photo_info_different_ids_produce_different_keys() {
        let key1 = photo_info(PhotoId(1), UserId(7));
        let key2 = photo_info(PhotoId(2), UserId(7));
        assert_ne!(key1, key2);
    }

    #[test]
    fn photo_info_different_viewers_produce_different_keys() {
        let key1 = photo_info(PhotoId(1), UserId(7));
        let key2 = photo_info(PhotoId(1), UserId(8));
        assert_ne!(key1, key2);
    }
}
