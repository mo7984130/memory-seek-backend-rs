use entities::photo::comment::CommentId;

pub fn likes_count(comment_id: CommentId) -> String {
    // photo:comment_like:likes_count:{comment_id}
    format!("p:cl:lc:{}", comment_id.0)
}

pub fn dirty_comment() -> &'static str {
    // photo:comment_like:dirty_comment
    "p:cl:dc"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn likes_count_returns_correct_format() {
        let key = likes_count(CommentId(42));
        assert_eq!(key, "p:cl:lc:42");
    }

    #[test]
    fn likes_count_different_ids_produce_different_keys() {
        let key1 = likes_count(CommentId(1));
        let key2 = likes_count(CommentId(2));
        assert_ne!(key1, key2);
    }

    #[test]
    fn dirty_comment_returns_correct_value() {
        assert_eq!(dirty_comment(), "p:cl:dc");
    }
}
