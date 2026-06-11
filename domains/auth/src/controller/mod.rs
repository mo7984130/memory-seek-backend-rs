mod auth_controller;

pub use auth_controller::AuthController;

use std::sync::Arc;

use axum::Router;
use common::traits::controller::ControllerRouter;

use crate::AuthState;

pub struct Controller;

impl ControllerRouter for Controller {
    type State = AuthState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        AuthController::protected_routes()
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        AuthController::public_routes()
    }
}
