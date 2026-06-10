use entities::photo::comment::CommentId;

pub fn likes_count(comment_id: CommentId) -> String {
    // photo:comment_like:likes_count:{comment_id}
    format!("p:cl:lc:{}", comment_id.0)
}

pub fn dirty_comment() -> &'static str {
    // photo:comment_like:dirty_comment
    "p:cl:dc"
}
