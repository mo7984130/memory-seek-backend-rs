pub mod email;
#[cfg(feature = "face-engine")]
pub mod face_engine;
#[cfg(feature = "s3")]
pub mod s3;
pub mod token_cipher;

use crate::config::AppConfig;
use crate::state::AppLibs;

pub struct AppLibsInit;

impl AppLibsInit {
    pub async fn init(cfg: &AppConfig) -> Result<AppLibs, common::error::AppError> {
        // 初始化 Email 客户端
        let email_client = email::init(&cfg.smtp);

        // 初始化 TokenCipher
        let token_cipher = token_cipher::init(&cfg.token_cipher);

        // 初始化 S3（如果启用）
        #[cfg(feature = "s3")]
        let s3_client = s3::init(cfg.s3.as_ref().expect("未配置s3"));

        // 初始化人脸模型
        #[cfg(feature = "face-engine")]
        let face_engine = face_engine::init(&cfg.face_engine);

        Ok(AppLibs {
            email_client,
            token_cipher,
            #[cfg(feature = "s3")]
            s3_client,
            #[cfg(feature = "face-engine")]
            face_engine,
        })
    }
}
