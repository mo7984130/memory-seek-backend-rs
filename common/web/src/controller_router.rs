use std::sync::Arc;

use axum::Router;

pub trait ControllerRouter {
    type State: Send + Sync + 'static;

    fn protected_routes() -> Router<Arc<Self::State>>;
    fn public_routes() -> Router<Arc<Self::State>>;

    fn routes() -> Router<Arc<Self::State>> {
        Self::public_routes().merge(Self::protected_routes())
    }
}
