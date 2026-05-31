use entities::{auth::user::UserId, photo::photo::PhotoId};

/// 生成用户收藏集合的 Redis 缓存键
///
/// # 参数
/// - `user_id`: 用户 ID
///
/// # 返回
/// 格式为 `p:u:f:c:i:{user_id}` 的缓存键
#[inline]
pub fn favorite_collection_id(user_id: UserId) -> String {
    //photo_entities:user:favorite:collection:id
    format!("p:u:f:c:i:{}", user_id.0)
}

/// 生成照片信息的 Redis 缓存键
///
/// # 参数
/// - `photo_id`: 照片 ID
///
/// # 返回
/// 格式为 `p:i:{photo_id}` 的缓存键
#[inline]
pub fn photo_info(photo_id: PhotoId) -> String {
    //photo_entities:info
    format!("p:i:{}", photo_id.0)
}

// /// 生成人脸人物名称的 Redis 缓存键
// ///
// /// # 参数
// /// - `person_id`: 人物 ID
// ///
// /// # 返回
// /// 格式为 `p:f:p:n:{person_id}` 的缓存键
// #[inline]
// pub fn face_person_name(person_id: PersonId) -> String {
//     //photo_entities:face:person:name
//     format!("p:f:p:n:{}", person_id.0)
// }
