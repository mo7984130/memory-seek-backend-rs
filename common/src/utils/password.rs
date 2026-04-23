use std::str::FromStr;

use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
};
use bcrypt;
use crate::error::AppError;
use crate::utils::ResultExt;
use password_hash::rand_core::OsRng;
use password_hash::SaltString;
use tracing::error;

/// 哈希算法类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum HashAlgorithm {
    Bcrypt(BcryptConfig),
    Argon2id(Argon2idConfig),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BcryptConfig {
    pub cost: u32
}

#[derive(Debug, Clone, PartialEq)]
pub struct Argon2idConfig {
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32
}

impl HashAlgorithm {
    pub fn detect(hash: &str) -> Option<Self> {
        if hash.starts_with("$2") {
            let cost = bcrypt::HashParts::from_str(hash).ok()?.get_cost();
            Some(Self::Bcrypt(BcryptConfig { cost }))
        } else if hash.starts_with("$argon2") {
            let parsed = PasswordHash::new(hash).ok()?;
            let m_cost = parsed.params.get_decimal("m")?;
            let t_cost = parsed.params.get_decimal("t")?;
            let p_cost = parsed.params.get_decimal("p")?;
            Some(Self::Argon2id(Argon2idConfig { m_cost, t_cost, p_cost }))
        } else {
            None
        }
    }

    pub fn hash(&self, password: &str) -> Result<String, AppError> {
        match self {
            Self::Bcrypt(cfg) => {
                bcrypt::hash(password, cfg.cost)
                    .trace_internal_err("bcrypt hash error", "Bcrypt 计算失败")
            },
            Self::Argon2id(cfg) => {
                Self::argon2_hasher(cfg)?
                    .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
                    .trace_internal_err("argon2id hash error", "Argon2id 计算失败")
                    .map(|h: PasswordHash| h.to_string())
            }
        }
    }

    pub fn verify(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        match self {
            Self::Bcrypt(_) => {
                bcrypt::verify(password, hash)
                    .trace_internal_err("bcrypt verify error", "Bcrypt 密码验证失败")
            },
            Self::Argon2id(cfg) => {
                let hasher = Self::argon2_hasher(cfg)?;
                let parsed = PasswordHash::new(hash)
                    .trace_internal_err("argon2 parse error", "解析 Argon2 哈希失败")?;
                match hasher.verify_password(password.as_bytes(), &parsed) {
                    Ok(()) => Ok(true),
                    Err(e) if matches!(e, password_hash::Error::Password) => Ok(false),
                    Err(e) => {
                        error!(reason = "argon2_verify_error", error = %e);
                        Err(AppError::InternalServerError)
                    }
                }
            }
        }
    }

    pub fn verify_and_detect(password: &str, hash: &str) -> Result<(bool, HashAlgorithm), AppError> {
        match HashAlgorithm::detect(hash) {
            Some(alg) => {
                let result = alg.verify(password, hash)?;
                Ok((result, alg))
            },
            None => {
                Err(())
                    .trace_internal_err("password_not_detect", "检测不到密码的算法")
            }
        }
    }

    fn argon2_hasher(cfg: &Argon2idConfig) -> Result<Argon2<'static>, AppError> {
        let params = Params::new(cfg.m_cost, cfg.t_cost, cfg.p_cost, None)
            .trace_internal_err("argon2_params_error", "创建 Argon2 参数失败")?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}
