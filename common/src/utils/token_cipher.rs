use aes_gcm::{
    AeadCore, Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::Sha256;

use crate::{error::AppError, ext::log_err};
use crate::{ext::ResultErrExt, models::ImageToken};

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
    /// 使用原始密钥和盐创建 TokenCipher 实例
    ///
    /// 内部通过 HKDF 从原始密钥派生 AES-256-GCM 加密密钥
    ///
    /// # 参数
    /// - `raw_key`: 用于密钥派生的原始密钥材料
    /// - `salt`: HKDF 密钥派生所需的盐值
    pub fn new(raw_key: impl AsRef<[u8]>, salt: impl AsRef<[u8]>) -> Self {
        let cipher = Self::build_cipher(raw_key.as_ref(), salt.as_ref());
        Self { cipher }
    }

    /// 从配置结构体创建 TokenCipher 实例
    ///
    /// # 参数
    /// - `config`: 包含 `key` 和 `salt` 字段的配置
    pub fn from_config(config: &TokenCipherConfig) -> Self {
        Self::new(&config.key, &config.salt)
    }

    /// 加密任意可序列化的 Payload 为 URL-safe Base64 token
    ///
    /// nonce_seed 参数控制 nonce 的生成方式：
    /// - 传入 `Some(seed)` 时，通过 HKDF 从 seed 派生确定性 nonce，相同 seed 产生相同密文
    /// - 传入 `None` 时，自动生成随机 nonce，每次加密结果不同
    ///
    /// # 参数
    /// - `payload`: 待加密的可序列化数据
    /// - `nonce_seed`: 可选的 nonce 种子，`Some` 实现确定性加密，`None` 使用随机 nonce
    ///
    /// # 返回
    /// 返回 URL-safe Base64 编码的加密 token
    ///
    /// # 错误
    /// - `AppError`: 序列化失败或 AES-GCM 加密失败
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
            .to_internal_err("token_serialize_error", "序列化 Payload 失败")?;
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_slice())
            .to_internal_err("aes_gcm_encrypt_error", "AES-GCM 加密失败")?;
        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(&combined))
    }

    /// 解密 URL-safe Base64 token 还原为原始数据
    ///
    /// # 参数
    /// - `token`: 由 `encrypt` 方法生成的加密 token 字符串
    ///
    /// # 返回
    /// 返回反序列化后的原始数据
    ///
    /// # 错误
    /// - `AppError`: Base64 解码失败、token 长度不合法、AES-GCM 解密失败或反序列化失败
    pub fn decrypt<T: DeserializeOwned>(&self, token: &str) -> Result<T, AppError> {
        let combined = URL_SAFE_NO_PAD
            .decode(token)
            .to_internal_err("token_base64_decode_error", "Token Base64 解码失败")?;
        if combined.len() <= NONCE_LEN {
            return Err(log_err(
                "token_too_short",
                "Token 长度不合法",
                "",
                AppError::InternalServerError,
            ));
        }
        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .to_internal_err("aes_gcm_decrypt_error", "AES-GCM 解密失败")?;
        serde_json::from_slice(&plaintext)
            .to_internal_err("token_deserialize_error", "反序列化 Payload 失败")
    }

    // 通过 HKDF 从原始密钥和盐派生 AES-256-GCM 密钥并创建加密器
    fn build_cipher(raw_key: &[u8], salt: &[u8]) -> Aes256Gcm {
        let hk = Hkdf::<Sha256>::new(Some(salt), raw_key);
        let mut derived = [0u8; 32];
        hk.expand(HKDF_KEY_INFO, &mut derived)
            .expect("HKDF expand 不会失败");
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&derived))
    }

    // 通过 HKDF 从种子字符串派生确定性 nonce
    fn derive_nonce(str: &str) -> [u8; NONCE_LEN] {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        Hkdf::<Sha256>::new(Some(HKDF_NONCE_SALT), str.as_bytes())
            .expand(HKDF_NONCE_INFO, &mut nonce_bytes)
            .expect("12 字节 HKDF expand 不会失败");
        nonce_bytes
    }
}

impl TokenCipher {
    pub fn encrypt_avatar_token(&self, avatar_file_id: Option<&str>) -> Option<String> {
        avatar_file_id.and_then(|key| {
            self.encrypt(&ImageToken::thumbnail(key), Some(key))
                .to_warn("encrypt_avatar_token_err", "加密头像失败", AppError::Ignore)
                .ok()
        })
    }
}
