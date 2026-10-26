pub mod user_controller;

pub use user_controller::UserController;

use std::sync::Arc;

use axum::Router;
use common_web::controller_router::ControllerRouter;

use crate::UserState;

pub struct Controller;

impl ControllerRouter for Controller {
    type State = UserState;

    /// 用户模块置于 `/user` 下的受保护路由
    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/user", UserController::protected_routes())
    }

    /// 用户模块置于 `/user` 下的公开路由
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/user", UserController::public_routes())
    }
}
