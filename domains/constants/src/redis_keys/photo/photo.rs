use types::{auth::user::UserId, photo::photo::PhotoId};

/// 生成照片信息的 Redis 缓存键
///
/// 缓存内容为 `PhotoRecord`，不含按浏览者签发的 token（token 在读取时按浏览者
/// 动态生成），因此键只按照片拆分，同一照片所有浏览者共享一份缓存。
///
/// # 参数
/// - `photo_id`: 照片 ID
///
/// # 返回
/// 格式为 `p:p:i:{photo_id}` 的缓存键
#[inline]
pub fn photo_info(photo_id: PhotoId) -> String {
    //photo:photo:info
    format!("p:p:i:{}", photo_id)
}

/// 生成照片尺寸缓存的 Redis 缓存键
///
/// 缓存内容为 `(width, height)` 元组，按 file_id 拆分（尺寸是照片的静态元数据）。
///
/// # 参数
/// - `file_id`: 照片文件 ID
///
/// # 返回
/// 格式为 `p:p:d:{file_id}` 的缓存键
#[inline]
pub fn photo_dimensions(file_id: &str) -> String {
    //photo:photo:dimensions
    format!("p:p:d:{}", file_id)
}

/// 生成照片 MD5 去重缓存的 Redis 缓存键
///
/// 缓存内容为布尔值（该 MD5 是否已存在），用于上传时快速去重。
///
/// # 参数
/// - `md5`: 照片文件 MD5 哈希
///
/// # 返回
/// 格式为 `p:p:m:{md5}` 的缓存键
#[inline]
pub fn photo_md5(md5: &str) -> String {
    //photo:photo:md5
    format!("p:p:m:{}", md5)
}

/// 生成用户对照片点赞状态的 Redis 缓存键。
///
/// 点赞状态取决于浏览者，因此键同时包含用户和照片 ID。
///
/// # 返回
/// 格式为 `p:p:l:{user_id}:{photo_id}` 的缓存键
#[inline]
pub fn photo_is_liked(user_id: UserId, photo_id: PhotoId) -> String {
    //photo:photo:is_liked
    format!("p:p:l:{}:{}", user_id, photo_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_info_returns_correct_format() {
        let key = photo_info(PhotoId(42));
        assert_eq!(key, "p:p:i:42");
    }

    #[test]
    fn photo_info_different_ids_produce_different_keys() {
        let key1 = photo_info(PhotoId(1));
        let key2 = photo_info(PhotoId(2));
        assert_ne!(key1, key2);
    }

    #[test]
    fn photo_dimensions_returns_correct_format() {
        let key = photo_dimensions("photos/2024/01/01/abc.jpg");
        assert_eq!(key, "p:p:d:photos/2024/01/01/abc.jpg");
    }

    #[test]
    fn photo_md5_returns_correct_format() {
        let key = photo_md5("d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(key, "p:p:m:d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn photo_is_liked_returns_correct_format() {
        let key = photo_is_liked(UserId(7), PhotoId(42));
        assert_eq!(key, "p:p:l:7:42");
    }
}
