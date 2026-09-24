//! 工具门面:口令/令牌等密码学能力已下沉 `common-crypto`,此处重导出以保持
//! `common::utils::token_cipher` / `common::utils::password_hash` 等既有路径不变。

mod db_utils;
mod temp_file;
mod type_map;

pub use db_utils::DbUtils;
pub use temp_file::{TempFile, remove_dir_all};
pub use type_map::TypeMap;

pub mod table_metadata;

pub use common_crypto::{Argon2idConfig, BcryptConfig, HashAlgorithm};
pub use common_crypto::{TokenCipher, TokenCipherConfig, init_token_cipher};
pub use common_crypto::{password_hash, rand_utils, token_cipher};

#[cfg(feature = "snowflake")]
pub use common_crypto::snowflake;

#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "metrics")]
pub use metrics::{GaugeGuard, MetricsTimer, MetricsTimerExt};
