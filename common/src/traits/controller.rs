use std::sync::Arc;

use axum::Router;

pub trait Controller {
    type State: Send + Sync + 'static;

    fn protected_routes() -> Router<Arc<Self::State>>;
    fn public_routes() -> Router<Arc<Self::State>>;
}
