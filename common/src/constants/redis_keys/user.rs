#[inline]
pub fn user_access_token(user_id: i64) -> String {
    //auth:user:accessToken
    format!("a:u:at:{}", user_id)
}
#[inline]
pub fn email_verify_code(email: &str) -> String {
    //auth:verify:email
    format!("a:v:e:{}", email)
}
#[inline]
pub fn inviter_code(code: &str) -> String {
    //auth:inviter:code
    format!("a:i:c:{}", code)
}
#[inline]
pub fn user_info_cache(user_id: i64) -> String {
    //user:info
    format!("u:i:{}", user_id)
}