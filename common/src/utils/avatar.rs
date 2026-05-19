use tracing::warn;

use crate::models::ImageToken;
use crate::utils::TokenCipher;

/// 加密头像 file_id 为可公开访问的 token
///
/// 使用 thumbnail 类型的 ImageToken 进行加密。当 file_id 为 `None` 或
/// 加密失败时返回 `None`，失败时仅记录警告日志不向上抛出错误。
///
/// # 参数
/// - `avatar_file_id`: 可选的头像文件 ID
/// - `token_cipher`: Token 加密器实例
///
/// # 返回
/// 返回加密后的 token 字符串；file_id 为 None 或加密失败时返回 `None`
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
