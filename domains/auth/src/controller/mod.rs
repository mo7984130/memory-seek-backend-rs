mod auth_controller;

pub use auth_controller::AuthController;

use std::sync::Arc;

use axum::Router;
use common_web::controller_router::ControllerRouter;

use crate::AuthState;

pub struct Controller;

impl ControllerRouter for Controller {
    type State = AuthState;

    /// 返回挂载在 `/auth` 下的受保护路由
    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/auth", AuthController::protected_routes())
    }

    /// 返回挂载在 `/auth` 下的公开路由
    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/auth", AuthController::public_routes())
    }
}
