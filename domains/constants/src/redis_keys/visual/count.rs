use types_core::{PersonId, UserId};

/// 用户“我喜欢”列表的总量缓存键。
#[inline]
pub fn liked_visuals(user_id: UserId) -> String {
    format!("v:c:l:{user_id}")
}

/// 某个人物关联影像总量缓存键。
#[inline]
pub fn person_visuals(person_id: PersonId) -> String {
    format!("v:c:p:{person_id}")
}

/// 含未分配人脸的影像总量缓存键。
pub const UNASSIGNED_FACE_VISUALS: &str = "v:c:u";

/// 人物列表总量缓存键。
pub const PERSONS: &str = "v:c:persons";

/// 人物搜索结果总量缓存键。
#[inline]
pub fn person_search(keyword: &str) -> String {
    format!("v:c:ps:{}", blake3::hash(keyword.as_bytes()).to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_keys_include_their_scope() {
        assert_eq!(liked_visuals(UserId(7)), "v:c:l:7");
        assert_eq!(person_visuals(PersonId(3)), "v:c:p:3");
        assert_eq!(UNASSIGNED_FACE_VISUALS, "v:c:u");
        assert_eq!(PERSONS, "v:c:persons");
        assert_ne!(person_search("alice"), person_search("bob"));
    }
}
