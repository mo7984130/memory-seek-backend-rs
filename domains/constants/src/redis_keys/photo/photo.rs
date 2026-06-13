use entities::photo::photo::PhotoId;

/// 生成照片信息的 Redis 缓存键
///
/// # 参数
/// - `photo_id`: 照片 ID
///
/// # 返回
/// 格式为 `p:i:{photo_id}` 的缓存键
#[inline]
pub fn photo_info(photo_id: PhotoId) -> String {
    //photo:photo:info
    format!("p:p:i:{}", photo_id.0)
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
}
