use types::photo::person::PersonId;

/// 生成人物信息的 Redis 缓存键
///
/// 缓存内容为轻量的人物摘要（不含人脸向量），按人物 ID 拆分。
///
/// # 参数
/// - `person_id`: 人物 ID
///
/// # 返回
/// 格式为 `p:person:i:{person_id}` 的缓存键
#[inline]
pub fn person_info(person_id: PersonId) -> String {
    //photo:person:info
    format!("p:person:i:{}", person_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_info_returns_correct_format() {
        let key = person_info(PersonId(42));
        assert_eq!(key, "p:person:i:42");
    }

    #[test]
    fn person_info_different_ids_produce_different_keys() {
        let key1 = person_info(PersonId(1));
        let key2 = person_info(PersonId(2));
        assert_ne!(key1, key2);
    }
}
