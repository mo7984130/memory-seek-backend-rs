use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("加密失败")]
    EncryptError,
    #[error("解密失败")]
    DecryptError,
    #[error("无效的token")]
    InvalidToken,
}
