use std::sync::Arc;

use axum::Router;
use common::utils::TokenCipher;
use deadpool_redis::Pool;
use oss::S3Client;
use photo::{CollectionController, CommentController, PhotoController, PhotoState, TimelineController};
use sea_orm::DatabaseConnection;
use crate::config::AppConfig;

#[cfg(feature = "face_recognition")]
use serde::Deserialize;

#[cfg(feature = "face_recognition")]
#[derive(Clone, Deserialize)]
pub struct FaceRecognitionConfig {
    det_model_path: String,
    rec_model_path: String
}

/// 初始化照片模块
///
/// 创建照片模块状态。当启用 `face_recognition` feature 时，会额外初始化
/// 人脸识别引擎（预热模型、启动清理任务、创建异步人脸处理通道）。
///
/// # 参数
/// - `cfg`: 应用配置，包含人脸识别模型路径（可选）
/// - `db`: 数据库连接
/// - `redis`: Redis 连接池
/// - `s3_client`: OSS 存储客户端
/// - `token_cipher`: Token 加解密器
///
/// # 返回
/// 返回封装好的照片状态 `Arc<PhotoState>`
pub async fn init_photo(
    #[cfg_attr(not(feature = "face_recognition"), allow(unused_variables))]
    cfg: &AppConfig,
    db: DatabaseConnection,
    redis: Pool,
    s3_client: Arc<S3Client>,
    token_cipher: Arc<TokenCipher>,
) -> Arc<PhotoState> {
    #[cfg(feature = "face_recognition")]
    let face_tx = {
        use face_engine::LazyFaceEngine;
        use tokio::sync::mpsc;

        let (face_tx, face_rx): (mpsc::Sender<photo::FaceTask>, mpsc::Receiver<photo::FaceTask>) = mpsc::channel(100);

        let lazy_engine = Arc::new(LazyFaceEngine::new(
            &cfg.face_recognition_config.det_model_path,
            &cfg.face_recognition_config.rec_model_path
        ));

        lazy_engine.warmup_on_startup().await
            .expect("人脸识别模型预热失败");

        lazy_engine.clone().start_cleanup_task();

        let db_for_face = db.clone();
        let lazy_engine_clone = lazy_engine.clone();
        tokio::spawn(async move {
            photo::FaceService::process_face_tasks(&db_for_face, face_rx, lazy_engine_clone).await;
        });

        face_tx
    };

    Arc::new(
        PhotoState::new(
            db,
            redis,
            s3_client,
            #[cfg(feature = "face_recognition")]
            face_tx,
            token_cipher
        )
    )
}

/// 挂载照片模块的公开路由
///
/// 将照片图片访问等无需认证的路由挂载到 `/photo/image` 路径下。
///
/// # 参数
/// - `router`: 已有的路由
/// - `photo_state`: 照片模块状态
///
/// # 返回
/// 返回挂载了照片公开路由的新路由
pub fn mount_public(
    router: Router,
    photo_state: Arc<PhotoState>,
) -> Router {
    router
        .nest("/photo", PhotoController::public_routes().with_state(photo_state.clone()))
}

/// 挂载照片模块的受保护路由
///
/// 将照片、收藏、评论、时间线等需要认证的路由挂载到对应路径下。
/// 启用 `face_recognition` feature 时额外挂载人脸识别路由。
///
/// # 参数
/// - `router`: 已有的路由
/// - `photo_state`: 照片模块状态
///
/// # 返回
/// 返回挂载了照片受保护路由的新路由
pub fn mount_protected(
    mut router: Router,
    photo_state: Arc<PhotoState>,
) -> Router {
    router = router
        .nest("/photo", PhotoController::routes().with_state(photo_state.clone()))
        .nest("/photo/collections", CollectionController::routes().with_state(photo_state.clone()))
        .nest("/photo/comment", CommentController::routes().with_state(photo_state.clone()))
        .nest("/photo/timeline", TimelineController::routes().with_state(photo_state.clone()));

    #[cfg(feature = "face_recognition")]
    {
        use photo::FaceController;

        router = router.nest("/photo/face", FaceController::routes().with_state(photo_state.clone()));
    }

    router
}
