//! 密码与令牌能力。
//!
//! - [`password_hash`]：口令哈希(Argon2id / Bcrypt)
//! - [`token_cipher`]：访问令牌的 AEAD 加解密(`hkdf` 派生 + `aes-gcm`)
//! - [`rand_utils`]：随机标识串
//!
//! 依赖方向:只依赖 [`common_core`](错误/时间),不依赖任何其它能力 crate。

pub mod password_hash;
pub mod rand_utils;
pub mod token_cipher;

pub use password_hash::{Argon2idConfig, BcryptConfig, HashAlgorithm};
pub use token_cipher::{TokenCipher, TokenCipherConfig, init_token_cipher, token_cipher};
