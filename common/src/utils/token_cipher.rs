use std::sync::OnceLock;

use aes_gcm::{
    AeadCore, Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::Sha256;

use crate::error::{AppError, DeferredError, DeferredResult};

const NONCE_LEN: usize = 12;
const HKDF_KEY_INFO: &[u8] = b"image-file-id-token-v1";
const HKDF_NONCE_INFO: &[u8] = b"nonce-v1";
const HKDF_NONCE_SALT: &[u8] = b"nonce-salt";

/// 全局唯一的 token 加密器，由启动时 `init_token_cipher` 写入
static TOKEN_CIPHER: OnceLock<TokenCipher> = OnceLock::new();

/// 初始化全局 token 加密器，幂等且线程安全
pub fn init_token_cipher(config: &TokenCipherConfig) -> &'static TokenCipher {
    TOKEN_CIPHER.get_or_init(|| TokenCipher::from_config(config))
}

/// 获取全局 token 加密器
pub fn token_cipher() -> &'static TokenCipher {
    TOKEN_CIPHER
        .get()
        .expect("token_cipher 未初始化，请先调用 common::utils::init_token_cipher")
}

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
    ) -> DeferredResult<String> {
        let nonce_bytes = match nonce_seed {
            Some(seed) => Self::derive_nonce(seed),
            None => {
                let n = Aes256Gcm::generate_nonce(&mut OsRng);
                n.into()
            }
        };
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = serde_json::to_vec(payload).map_err(|error| {
            DeferredError::error(
                "token_serialize_error",
                "序列化 Payload 失败",
                error,
                AppError::InternalServerError,
            )
        })?;
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_slice())
            .map_err(|error| {
                DeferredError::error(
                    "aes_gcm_encrypt_error",
                    "AES-GCM 加密失败",
                    error,
                    AppError::InternalServerError,
                )
            })?;
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
    pub fn decrypt<T: DeserializeOwned>(&self, token: &str) -> DeferredResult<T> {
        let combined = URL_SAFE_NO_PAD.decode(token).map_err(|error| {
            DeferredError::error(
                "token_base64_decode_error",
                "Token Base64 解码失败",
                error,
                AppError::InternalServerError,
            )
        })?;
        if combined.len() <= NONCE_LEN {
            return Err(DeferredError::error_without_source(
                "token_too_short",
                "Token 长度不合法",
                AppError::InternalServerError,
            ));
        }
        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self.cipher.decrypt(nonce, ciphertext).map_err(|error| {
            DeferredError::error(
                "aes_gcm_decrypt_error",
                "AES-GCM 解密失败",
                error,
                AppError::InternalServerError,
            )
        })?;
        serde_json::from_slice(&plaintext).map_err(|error| {
            DeferredError::error(
                "token_deserialize_error",
                "反序列化 Payload 失败",
                error,
                AppError::InternalServerError,
            )
        })
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

impl TokenCipher {}

#[cfg(test)]
mod tests {
    use super::*;

    /// 结构化 payload 加密解密往返测试用（替代原 ImageToken 用例）
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestPayload {
        file_id: String,
        score: f32,
        tags: Vec<String>,
    }

    fn test_cipher() -> TokenCipher {
        TokenCipher::new("test-key-for-unit-tests", "test-salt")
    }

    // --- 加密解密往返 ---

    #[test]
    fn test_encrypt_decrypt_roundtrip_string() {
        let cipher = test_cipher();
        let payload = "hello world".to_string();
        let token = cipher.encrypt(&payload, Some("seed")).unwrap();
        let decrypted: String = cipher.decrypt(&token).unwrap();
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip_struct() {
        let cipher = test_cipher();
        let payload = TestPayload {
            file_id: "file-xyz".to_string(),
            score: 0.95,
            tags: vec!["face".to_string(), "crop".to_string()],
        };
        let token = cipher.encrypt(&payload, Some("seed")).unwrap();
        let decrypted: TestPayload = cipher.decrypt(&token).unwrap();
        assert_eq!(decrypted, payload);
    }

    // --- 确定性 nonce ---

    #[test]
    fn test_deterministic_nonce_same_seed() {
        let cipher = test_cipher();
        let payload = "deterministic".to_string();
        let token1 = cipher.encrypt(&payload, Some("same-seed")).unwrap();
        let token2 = cipher.encrypt(&payload, Some("same-seed")).unwrap();
        assert_eq!(token1, token2);
    }

    #[test]
    fn test_deterministic_nonce_different_seed() {
        let cipher = test_cipher();
        let payload = "deterministic".to_string();
        let token1 = cipher.encrypt(&payload, Some("seed-a")).unwrap();
        let token2 = cipher.encrypt(&payload, Some("seed-b")).unwrap();
        assert_ne!(token1, token2);
    }

    // --- 随机 nonce ---

    #[test]
    fn test_random_nonce_no_seed() {
        let cipher = test_cipher();
        let payload = "random".to_string();
        let token1 = cipher.encrypt(&payload, None).unwrap();
        let token2 = cipher.encrypt(&payload, None).unwrap();
        assert_ne!(token1, token2);
    }

    // --- 不同 key 解密失败 ---

    #[test]
    fn test_decrypt_with_wrong_key() {
        let cipher1 = TokenCipher::new("key-1", "salt");
        let cipher2 = TokenCipher::new("key-2", "salt");
        let payload = "secret".to_string();
        let token = cipher1.encrypt(&payload, Some("seed")).unwrap();
        let result: DeferredResult<String> = cipher2.decrypt(&token);
        assert!(result.is_err());
    }

    // --- 无效 token 处理 ---

    #[test]
    fn test_decrypt_invalid_base64() {
        let cipher = test_cipher();
        let result: DeferredResult<String> = cipher.decrypt("not-valid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_token_too_short() {
        let cipher = test_cipher();
        // 编码一个长度 <= NONCE_LEN 的 bytes
        let short_bytes = vec![0u8; NONCE_LEN]; // 刚好等于 NONCE_LEN，应该 <= NONCE_LEN
        let short_token = URL_SAFE_NO_PAD.encode(&short_bytes);
        let result: DeferredResult<String> = cipher.decrypt(&short_token);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_empty_token() {
        let cipher = test_cipher();
        let result: DeferredResult<String> = cipher.decrypt("");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_corrupted_ciphertext() {
        let cipher = test_cipher();
        let payload = "data".to_string();
        let token = cipher.encrypt(&payload, Some("seed")).unwrap();
        // 篡改 token 的最后几个字符
        let mut corrupted = token.clone();
        let last = corrupted.pop().unwrap();
        corrupted.push(if last == 'A' { 'B' } else { 'A' });
        let result: DeferredResult<String> = cipher.decrypt(&corrupted);
        assert!(result.is_err());
    }

    // --- from_config ---

    #[test]
    fn test_from_config() {
        let config = TokenCipherConfig {
            key: "config-key".to_string(),
            salt: "config-salt".to_string(),
        };
        let cipher = TokenCipher::from_config(&config);
        let payload = "test".to_string();
        let token = cipher.encrypt(&payload, Some("seed")).unwrap();
        let decrypted: String = cipher.decrypt(&token).unwrap();
        assert_eq!(decrypted, payload);
    }

    // --- 全局静态单例 ---

    #[test]
    fn test_global_token_cipher() {
        init_token_cipher(&TokenCipherConfig {
            key: "global-key".to_string(),
            salt: "global-salt".to_string(),
        });
        let payload = "global".to_string();
        let token = token_cipher().encrypt(&payload, Some("seed")).unwrap();
        let decrypted: String = token_cipher().decrypt(&token).unwrap();
        assert_eq!(decrypted, payload);
    }
}
