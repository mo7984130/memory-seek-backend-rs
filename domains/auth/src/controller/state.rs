use deadpool_redis::Pool;
use email::EmailClient;
use sea_orm::DatabaseConnection;

use img_url_generator::EncryptionKey;

pub struct AuthState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub email_client: EmailClient,
    pub encryption_key: EncryptionKey,
}
