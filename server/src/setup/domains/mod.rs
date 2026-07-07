#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "user")]
pub mod user;

#[cfg(feature = "photo")]
pub mod photo;

#[cfg(feature = "backup")]
pub mod backup;

use crate::config::AppConfig;
use crate::state::AppState;
use axum::Router;
use std::sync::Arc;

pub struct AppDomains;

impl AppDomains {
    #[allow(unused_mut)]
    pub fn init(
        _state: &Arc<AppState>,
        _cfg: &AppConfig,
    ) -> (Router<Arc<AppState>>, Router<Arc<AppState>>) {
        let mut public_router = Router::new();
        let mut protected_router = Router::new();

        #[cfg(feature = "auth")]
        {
            let (pub_r, prot_r) = auth::register(_state, _cfg);
            public_router = public_router.merge(pub_r);
            protected_router = protected_router.merge(prot_r);
        }

        #[cfg(feature = "user")]
        {
            let (pub_r, prot_r) = user::register(_state, _cfg);
            public_router = public_router.merge(pub_r);
            protected_router = protected_router.merge(prot_r);
        }

        #[cfg(feature = "photo")]
        {
            let (pub_r, prot_r) = photo::register(_state, _cfg);
            public_router = public_router.merge(pub_r);
            protected_router = protected_router.merge(prot_r);
        }

        #[cfg(feature = "backup")]
        {
            let (pub_r, prot_r) = backup::register(_state, _cfg);
            public_router = public_router.merge(pub_r);
            protected_router = protected_router.merge(prot_r);
        }

        (public_router, protected_router)
    }
}
