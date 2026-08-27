#[derive(Clone)]
pub struct AuditState {
    pub db: sea_orm::DatabaseConnection,
}
