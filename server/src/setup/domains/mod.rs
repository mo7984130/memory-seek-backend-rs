#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "backup")]
pub mod backup;

#[cfg(feature = "audit")]
pub mod audit;

#[cfg(feature = "user")]
pub mod user;

#[cfg(feature = "photo")]
pub mod photo;

#[linkme::distributed_slice]
pub static APP_DOMAINS_FIRST: [crate::setup::InitFn];
#[linkme::distributed_slice]
pub static APP_DOMAINS: [crate::setup::InitFn];
