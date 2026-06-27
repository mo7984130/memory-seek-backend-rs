use std::sync::Arc;

use axum::Router;

use crate::state::PhotoState;

pub mod collection_controller;
pub mod collection_photo_controller;
pub mod comment_controller;
pub mod comment_like_controller;
pub mod photo_controller;
pub mod photo_like_controller;
pub mod timeline_stat_controller;

use collection_controller::CollectionController;
use collection_photo_controller::CollectionPhotoController;
use comment_controller::CommentController;
use photo_controller::PhotoController;
use photo_like_controller::PhotoLikeController;
use timeline_stat_controller::TimelineStatController;

use common::traits::controller::ControllerRouter;

pub struct Controller;

impl ControllerRouter for Controller {
    type State = PhotoState;

    /// photo 模块的公开路由（图片访问等无需认证的接口）
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/photo", PhotoController::public_routes())
    }

    /// photo 模块的受保护路由（需要认证的接口）
    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new()
            .nest("/photo", PhotoController::protected_routes())
            .nest(
                "/photo/collections",
                CollectionController::protected_routes()
                    .merge(CollectionPhotoController::protected_routes()),
            )
            .nest("/photo/comment", CommentController::protected_routes())
            .nest("/photo/likes", PhotoLikeController::protected_routes())
            .nest(
                "/photo/timeline",
                TimelineStatController::protected_routes(),
            )
    }
}
