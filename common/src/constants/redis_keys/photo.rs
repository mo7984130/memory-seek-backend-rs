#[inline]
pub fn favorite_collection_id(user_id: i64) -> String {
    //photo_entities:user:favorite:collection:id
    format!("p:u:f:c:i:{}", user_id)
}

#[inline]
pub fn photo_info(photo_id: i64) -> String {
    //photo_entities:info
    format!("p:i:{}", photo_id)
}

#[inline]
pub fn face_person_name(person_id: i64) -> String {
    //photo_entities:face:person:name
    format!("p:f:p:n:{}", person_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photo_redis_keys() {
        // 常规值
        assert_eq!(favorite_collection_id(1), "p:u:f:c:i:1");
        // 零值
        assert_eq!(favorite_collection_id(0), "p:u:f:c:i:0");
        // 大整数 (确保 i64 范围正常)
        assert_eq!(favorite_collection_id(9223372036854775807), "p:u:f:c:i:9223372036854775807");

        assert_eq!(photo_info(1), "p:i:1");
        assert_eq!(photo_info(0), "p:i:0");
        assert_eq!(photo_info(9223372036854775807), "p:i:9223372036854775807");

        // 人脸人物名称缓存
        assert_eq!(face_person_name(1), "p:f:p:n:1");
        assert_eq!(face_person_name(0), "p:f:p:n:0");
        assert_eq!(face_person_name(9223372036854775807), "p:f:p:n:9223372036854775807");
    }
}