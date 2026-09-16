use std::sync::Arc;

use axum::Router;

use crate::state::VisualState;

pub mod collection_controller;
pub mod collection_visual_controller;
pub mod comment_controller;
pub mod comment_like_controller;
#[cfg(feature = "face")]
pub mod face_controller;
#[cfg(feature = "face")]
pub mod person_controller;
pub mod visual_controller;
pub mod visual_like_controller;
pub mod timeline_stat_controller;

use collection_controller::CollectionController;
use collection_visual_controller::CollectionVisualController;
use comment_controller::CommentController;
use comment_like_controller::CommentLikeController;
#[cfg(feature = "face")]
pub use face_controller::FaceController;
#[cfg(feature = "face")]
pub use person_controller::PersonController;
use visual_controller::VisualController;
use visual_like_controller::VisualLikeController;
use timeline_stat_controller::TimelineStatController;

use common::axum::controller_router::ControllerRouter;

pub struct Controller;

impl ControllerRouter for Controller {
    type State = VisualState;

    /// visual 模块的公开路由（影像访问等无需认证的接口）
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/visual", VisualController::public_routes())
    }

    /// visual 模块的受保护路由（需要认证的接口）
    fn protected_routes() -> Router<Arc<Self::State>> {
        let router = Router::new()
            .nest(
                "/visual",
                VisualController::protected_routes().merge(VisualLikeController::protected_routes()),
            )
            .nest(
                "/visual/collections",
                CollectionController::protected_routes()
                    .merge(CollectionVisualController::protected_routes()),
            )
            .nest(
                "/visual/comment",
                CommentController::protected_routes()
                    .merge(CommentLikeController::protected_routes()),
            )
            .nest(
                "/visual/timeline",
                TimelineStatController::protected_routes(),
            );

        #[cfg(feature = "face")]
        let router = router.nest("/visual/face", FaceController::protected_routes());
        #[cfg(feature = "face")]
        let router = router.nest("/visual/person", PersonController::protected_routes());

        router
    }
}
