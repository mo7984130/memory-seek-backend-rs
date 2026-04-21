use aes_gcm::{
    AeadCore, Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit, OsRng}
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::Sha256;

use crate::error::AppError;
use crate::utils::ResultExt;

const NONCE_LEN: usize = 12;
const HKDF_KEY_INFO: &[u8] = b"image-file-id-token-v1";
const HKDF_NONCE_INFO: &[u8] = b"nonce-v1";
const HKDF_NONCE_SALT: &[u8] = b"nonce-salt";

pub struct TokenCipher {
    cipher: Aes256Gcm,
}

#[derive(Clone, Deserialize)]
pub struct TokenCipherConfig {
    pub key: String,
    pub salt: String,
}

impl TokenCipher {
    pub fn new(raw_key: impl AsRef<[u8]>, salt: impl AsRef<[u8]>) -> Self {
        let cipher = Self::build_cipher(raw_key.as_ref(), salt.as_ref());
        Self { cipher }
    }

    pub fn from_config(config: &TokenCipherConfig) -> Self {
        Self::new(&config.key, &config.salt)
    }

    /// 加密任意可序列化的 Payload，nonce_seed 决定确定性
    /// - 需要稳定 URL（如 file_id）：传入 file_id 作为 nonce_seed
    /// - 需要随机性：传入 None，自动生成随机 nonce
    pub fn encrypt<T: Serialize>(
            &self,
            payload: &T,
            nonce_seed: Option<&str>,
    ) -> Result<String, AppError> {
        let nonce_bytes = match nonce_seed {
            Some(seed) => Self::derive_nonce(seed),
            None => {
                let n = Aes256Gcm::generate_nonce(&mut OsRng);
                n.into()
            }
        };
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = serde_json::to_vec(payload)
            .trace_internal_err("token_serialize_error", "序列化 Payload 失败")?;
        let ciphertext = self.cipher
            .encrypt(nonce, plaintext.as_slice())
            .trace_internal_err("aes_gcm_encrypt_error", "AES-GCM 加密失败")?;
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(&combined))
    }

    pub fn decrypt<T: DeserializeOwned>(&self, token: &str) -> Result<T, AppError> {
        let combined = URL_SAFE_NO_PAD
            .decode(token)
            .trace_internal_err("token_base64_decode_error", "Token Base64 解码失败")?;
        if combined.len() <= NONCE_LEN {
            return Err(()).trace_internal_err("token_too_short", "Token 长度不合法");
        }
        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .trace_internal_err("aes_gcm_decrypt_error", "AES-GCM 解密失败")?;
        serde_json::from_slice(&plaintext)
            .trace_internal_err("token_deserialize_error", "反序列化 Payload 失败")
    }

    fn build_cipher(raw_key: &[u8], salt: &[u8]) -> Aes256Gcm {
        let hk = Hkdf::<Sha256>::new(Some(salt), raw_key);
        let mut derived = [0u8; 32];
        hk.expand(HKDF_KEY_INFO, &mut derived)
            .expect("HKDF expand 不会失败");
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&derived))
    }

    fn derive_nonce(str: &str) -> [u8; NONCE_LEN] {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        Hkdf::<Sha256>::new(Some(HKDF_NONCE_SALT), str.as_bytes())
            .expand(HKDF_NONCE_INFO, &mut nonce_bytes)
            .expect("12 字节 HKDF expand 不会失败");
        nonce_bytes
    }
}
