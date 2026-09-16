use std::sync::Arc;

use axum::Router;

use crate::state::MediaState;

pub mod collection_controller;
pub mod collection_media_controller;
pub mod comment_controller;
pub mod comment_like_controller;
#[cfg(feature = "face")]
pub mod face_controller;
#[cfg(feature = "face")]
pub mod person_controller;
pub mod media_controller;
pub mod media_like_controller;
pub mod timeline_stat_controller;

use collection_controller::CollectionController;
use collection_media_controller::CollectionMediaController;
use comment_controller::CommentController;
use comment_like_controller::CommentLikeController;
#[cfg(feature = "face")]
pub use face_controller::FaceController;
#[cfg(feature = "face")]
pub use person_controller::PersonController;
use media_controller::MediaController;
use media_like_controller::MediaLikeController;
use timeline_stat_controller::TimelineStatController;

use common::axum::controller_router::ControllerRouter;

pub struct Controller;

impl ControllerRouter for Controller {
    type State = MediaState;

    /// media 模块的公开路由（媒体访问等无需认证的接口）
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/media", MediaController::public_routes())
    }

    /// media 模块的受保护路由（需要认证的接口）
    fn protected_routes() -> Router<Arc<Self::State>> {
        let router = Router::new()
            .nest(
                "/media",
                MediaController::protected_routes().merge(MediaLikeController::protected_routes()),
            )
            .nest(
                "/media/collections",
                CollectionController::protected_routes()
                    .merge(CollectionMediaController::protected_routes()),
            )
            .nest(
                "/media/comment",
                CommentController::protected_routes()
                    .merge(CommentLikeController::protected_routes()),
            )
            .nest(
                "/media/timeline",
                TimelineStatController::protected_routes(),
            );

        #[cfg(feature = "face")]
        let router = router.nest("/media/face", FaceController::protected_routes());
        #[cfg(feature = "face")]
        let router = router.nest("/media/person", PersonController::protected_routes());

        router
    }
}
