#[inline]
pub fn favorite_collection_id(user_id: i64) -> String {
    //photo:user:favorite:collection:id
    format!("p:u:f:c:i:{}", user_id)
}

#[inline]
pub fn photo_info(photo_id: i64) -> String {
    //photo:info
    format!("p:i:{}", photo_id)
}