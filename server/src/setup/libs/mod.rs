#[cfg(feature = "face-engine")]
pub mod face_engine;
#[cfg(feature = "s3")]
pub mod s3;

pub mod token_cipher;

use crate::config::AppConfig;
use crate::state::AppLibs;

pub struct AppLibsInit;

impl AppLibsInit {
    pub async fn init(cfg: &AppConfig) -> anyhow::Result<AppLibs> {
        // 初始化 TokenCipher
        let token_cipher = token_cipher::init(cfg)?;

        // 初始化 S3（如果启用）
        #[cfg(feature = "s3")]
        let s3_client = s3::init(cfg);

        // 初始化人脸模型
        #[cfg(feature = "face-engine")]
        let face_engine = face_engine::init(cfg);

        Ok(AppLibs {
            token_cipher,
            #[cfg(feature = "s3")]
            s3_client,
            #[cfg(feature = "face-engine")]
            face_engine,
        })
    }
}
