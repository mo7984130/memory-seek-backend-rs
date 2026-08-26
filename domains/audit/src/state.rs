#[derive(Clone)]
#[cfg(feature = "controller")]
pub struct AuditState {
    pub db: sea_orm::DatabaseConnection,
}
