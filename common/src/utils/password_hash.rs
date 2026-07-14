use std::str::FromStr;

use crate::error::AppError;
use crate::ext::{ResultErrExt, log_err};
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use bcrypt;
use password_hash::SaltString;
use password_hash::rand_core::OsRng;
use tracing::error;

/// 哈希算法类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum HashAlgorithm {
    Bcrypt(BcryptConfig),
    Argon2id(Argon2idConfig),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BcryptConfig {
    pub cost: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Argon2idConfig {
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl HashAlgorithm {
    /// 从哈希字符串自动检测所使用的密码哈希算法及其参数
    ///
    /// # 参数
    /// - `hash`: 密码哈希字符串，支持 bcrypt（`$2` 前缀）和 Argon2id（`$argon2` 前缀）
    ///
    /// # 返回
    /// 返回检测到的算法类型及参数；无法识别时返回 `None`
    pub fn detect(hash: &str) -> Option<Self> {
        if hash.starts_with("$2") {
            let cost = bcrypt::HashParts::from_str(hash).ok()?.get_cost();
            Some(Self::Bcrypt(BcryptConfig { cost }))
        } else if hash.starts_with("$argon2") {
            let parsed = PasswordHash::new(hash).ok()?;
            let m_cost = parsed.params.get_decimal("m")?;
            let t_cost = parsed.params.get_decimal("t")?;
            let p_cost = parsed.params.get_decimal("p")?;
            Some(Self::Argon2id(Argon2idConfig {
                m_cost,
                t_cost,
                p_cost,
            }))
        } else {
            None
        }
    }

    /// 使用当前算法对明文密码进行哈希
    ///
    /// # 参数
    /// - `password`: 明文密码
    ///
    /// # 返回
    /// 返回哈希后的密码字符串
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: 哈希计算过程中发生内部错误
    pub fn hash(&self, password: &str) -> Result<String, AppError> {
        match self {
            Self::Bcrypt(cfg) => bcrypt::hash(password, cfg.cost)
                .trace_internal_err("bcrypt hash error", "Bcrypt 计算失败"),
            Self::Argon2id(cfg) => {
                let hash = Self::argon2_hasher(cfg)?
                    .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
                    .trace_internal_err("argon2id hash error", "Argon2id 计算失败")?
                    .to_string();
                Ok(hash)
            }
        }
    }

    /// 验证明文密码与哈希值是否匹配
    ///
    /// # 参数
    /// - `password`: 明文密码
    /// - `hash`: 存储的密码哈希值
    ///
    /// # 返回
    /// 匹配返回 `true`，不匹配返回 `false`
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: 哈希解析或验证过程中发生内部错误
    pub fn verify(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        match self {
            Self::Bcrypt(_) => bcrypt::verify(password, hash)
                .trace_internal_err("bcrypt verify error", "Bcrypt 密码验证失败"),
            Self::Argon2id(cfg) => {
                let hasher = Self::argon2_hasher(cfg)?;
                let parsed = PasswordHash::new(hash)
                    .trace_internal_err("argon2 parse error", "解析 Argon2 哈希失败")?;
                match hasher.verify_password(password.as_bytes(), &parsed) {
                    Ok(()) => Ok(true),
                    Err(password_hash::Error::Password) => Ok(false),
                    Err(e) => {
                        error!(reason = "argon2_verify_error", error = %e);
                        Err(AppError::InternalServerError)
                    }
                }
            }
        }
    }

    /// 自动检测哈希算法并验证密码是否匹配
    ///
    /// # 参数
    /// - `password`: 明文密码
    /// - `hash`: 存储的密码哈希值
    ///
    /// # 返回
    /// 返回 `(是否匹配, 检测到的算法类型)` 元组
    ///
    /// # 错误
    /// - `AppError`: 无法识别哈希算法或验证过程中发生错误
    pub fn verify_and_detect(
        password: &str,
        hash: &str,
    ) -> Result<(bool, HashAlgorithm), AppError> {
        match HashAlgorithm::detect(hash) {
            Some(alg) => {
                let result = alg.verify(password, hash)?;
                Ok((result, alg))
            }
            None => Err(log_err(
                "password_not_detect",
                "密码算法检测失败",
                "",
                AppError::InternalServerError,
            )),
        }
    }

    // 根据配置创建 Argon2id 哈希器实例
    fn argon2_hasher(cfg: &Argon2idConfig) -> Result<Argon2<'static>, AppError> {
        let params = Params::new(cfg.m_cost, cfg.t_cost, cfg.p_cost, None)
            .trace_internal_err("argon2_params_error", "创建 Argon2 参数失败")?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }

    /// 执行恒定时间的 dummy 验证，防止基于时序的用户枚举攻击
    ///
    /// 当用户不存在时调用此方法，使响应时间与密码错误时保持一致，
    /// 从而阻止攻击者通过响应时间差异枚举有效用户
    pub fn dummy_verify() {
        let _ = bcrypt::verify(
            "dummy",
            "$2b$12$QIgiYYcKC7dCwqhEmAX.duD4QA1t5Hgr9HAsmiawNdkXCdxZ8Dvea",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // $argon2id$v=19$m=16384,t=2,p=1$zcGSKX21GtoXbkIRxMLPXQ$QyhhvsEdkENJXKJS9LBaphiQX5nHQcc+w/MGdwUwYzQ
    fn print_test123456_hash() {
        let alg = HashAlgorithm::Argon2id(Argon2idConfig {
            m_cost: 16 * 1024,
            t_cost: 2,
            p_cost: 1,
        });
        let hash = alg.hash("Test123456").unwrap();
        println!("Test123456 hash: {}", hash.clone());
        assert!(alg.verify("Test123456", &hash).unwrap(), "验证失败")
    }

    #[test]
    fn detect_bcrypt_hash() {
        let alg = HashAlgorithm::Bcrypt(BcryptConfig { cost: 12 });
        let hash = alg.hash("test_password").unwrap();
        let detected = HashAlgorithm::detect(&hash);
        assert_eq!(
            detected,
            Some(HashAlgorithm::Bcrypt(BcryptConfig { cost: 12 }))
        );
    }

    #[test]
    fn detect_argon2id_hash() {
        let alg = HashAlgorithm::Argon2id(Argon2idConfig {
            m_cost: 16384,
            t_cost: 2,
            p_cost: 1,
        });
        let hash = alg.hash("test_password").unwrap();
        let detected = HashAlgorithm::detect(&hash);
        assert_eq!(
            detected,
            Some(HashAlgorithm::Argon2id(Argon2idConfig {
                m_cost: 16384,
                t_cost: 2,
                p_cost: 1,
            }))
        );
    }

    #[test]
    fn detect_invalid_hash_returns_none() {
        assert_eq!(HashAlgorithm::detect("not_a_valid_hash"), None);
        assert_eq!(HashAlgorithm::detect(""), None);
        assert_eq!(HashAlgorithm::detect("$scrypt$foo"), None);
    }

    #[test]
    fn bcrypt_hash_and_verify_roundtrip() {
        let alg = HashAlgorithm::Bcrypt(BcryptConfig { cost: 4 });
        let hash = alg.hash("my_secret").unwrap();
        assert!(alg.verify("my_secret", &hash).unwrap());
    }

    #[test]
    fn argon2id_hash_and_verify_roundtrip() {
        let alg = HashAlgorithm::Argon2id(Argon2idConfig {
            m_cost: 16384,
            t_cost: 2,
            p_cost: 1,
        });
        let hash = alg.hash("my_secret").unwrap();
        assert!(alg.verify("my_secret", &hash).unwrap());
    }

    #[test]
    fn verify_wrong_password_returns_false() {
        let bcrypt_alg = HashAlgorithm::Bcrypt(BcryptConfig { cost: 4 });
        let bcrypt_hash = bcrypt_alg.hash("correct_password").unwrap();
        assert!(!bcrypt_alg.verify("wrong_password", &bcrypt_hash).unwrap());

        let argon2_alg = HashAlgorithm::Argon2id(Argon2idConfig {
            m_cost: 16384,
            t_cost: 2,
            p_cost: 1,
        });
        let argon2_hash = argon2_alg.hash("correct_password").unwrap();
        assert!(!argon2_alg.verify("wrong_password", &argon2_hash).unwrap());
    }

    #[test]
    fn verify_and_detect_bcrypt() {
        let alg = HashAlgorithm::Bcrypt(BcryptConfig { cost: 4 });
        let hash = alg.hash("detect_me").unwrap();
        let (matched, detected_alg) = HashAlgorithm::verify_and_detect("detect_me", &hash).unwrap();
        assert!(matched);
        assert_eq!(
            detected_alg,
            HashAlgorithm::Bcrypt(BcryptConfig { cost: 4 })
        );
    }

    #[test]
    fn verify_and_detect_argon2id() {
        let alg = HashAlgorithm::Argon2id(Argon2idConfig {
            m_cost: 16384,
            t_cost: 2,
            p_cost: 1,
        });
        let hash = alg.hash("detect_me").unwrap();
        let (matched, detected_alg) = HashAlgorithm::verify_and_detect("detect_me", &hash).unwrap();
        assert!(matched);
        assert_eq!(
            detected_alg,
            HashAlgorithm::Argon2id(Argon2idConfig {
                m_cost: 16384,
                t_cost: 2,
                p_cost: 1,
            })
        );
    }

    #[test]
    fn dummy_verify_does_not_panic() {
        HashAlgorithm::dummy_verify();
    }
}
