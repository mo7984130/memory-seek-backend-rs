pub mod user_controller;

pub use user_controller::UserController;

use std::sync::Arc;

use axum::Router;
use common::axum::controller_router::ControllerRouter;

use crate::UserState;

pub struct Controller;

impl ControllerRouter for Controller {
    type State = UserState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/user", UserController::protected_routes())
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        Router::new().nest("/user", UserController::public_routes())
    }
}
