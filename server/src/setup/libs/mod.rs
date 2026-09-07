#[cfg(feature = "_email")]
pub mod email;
#[cfg(feature = "face-engine")]
pub mod face_engine;
#[cfg(feature = "_s3")]
pub mod s3;
#[cfg(feature = "_token_cipher")]
pub mod token_cipher;

use crate::setup::InitFn;

#[linkme::distributed_slice]
pub static APP_LIBS: [InitFn];
