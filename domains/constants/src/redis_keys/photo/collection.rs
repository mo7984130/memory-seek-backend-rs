use entities::auth::user::UserId;

/// 生成用户收藏集合的 Redis 缓存键
///
/// # 参数
/// - `user_id`: 用户 ID
///
/// # 返回
/// 格式为 `p:u:f:c:i:{user_id}` 的缓存键
#[inline]
pub fn favorite_collection_id(user_id: UserId) -> String {
    //photo:collection:favorite_collection_id
    format!("p:c:f_c_i:{}", user_id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorite_collection_id_returns_correct_format() {
        let key = favorite_collection_id(UserId(42));
        assert_eq!(key, "p:c:f_c_i:42");
    }

    #[test]
    fn favorite_collection_id_different_ids_produce_different_keys() {
        let key1 = favorite_collection_id(UserId(1));
        let key2 = favorite_collection_id(UserId(2));
        assert_ne!(key1, key2);
    }
}
