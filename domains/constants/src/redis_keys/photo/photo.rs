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
