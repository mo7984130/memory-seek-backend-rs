#[inline]
pub fn user_access_token(user_id: i64) -> String {
    format!("auth:user:accessToken:{}", user_id)
}
#[inline]
pub fn email_verify_code(email: &str) -> String {
    format!("auth:verify:email:{}", email)
}
#[inline]
pub fn inviter_code(code: &str) -> String {
    format!("auth:inviter:code:{}", code)
}
#[inline]
pub fn user_info_cache(user_id: i64) -> String {
    format!("auth:user:info:cache:{}", user_id)
}