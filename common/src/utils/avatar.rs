use tracing::warn;

use crate::models::ImageToken;
use crate::utils::TokenCipher;

/// 加密头像 file_id 为 token
/// 返回 None 如果 file_id 为 None 或加密失败
pub fn encrypt_avatar_token(
    avatar_file_id: Option<&str>,
    token_cipher: &TokenCipher,
) -> Option<String> {
    avatar_file_id.and_then(|key| {
        token_cipher
            .encrypt(&ImageToken::thumbnail(key.to_string()), Some(key))
            .inspect_err(|e| warn!(error = %e, "加密头像失败"))
            .ok()
    })
}
