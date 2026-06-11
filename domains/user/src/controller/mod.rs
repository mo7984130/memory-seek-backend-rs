pub mod user_controller;

pub use user_controller::UserController;

use std::sync::Arc;

use axum::Router;
use common::traits::controller::ControllerRouter;

use crate::UserState;

pub struct Controller;

impl ControllerRouter for Controller {
    type State = UserState;

    fn protected_routes() -> Router<Arc<Self::State>> {
        UserController::protected_routes()
    }

    fn public_routes() -> Router<Arc<Self::State>> {
        UserController::public_routes()
    }
}
