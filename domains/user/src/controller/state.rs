use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;

use img_url_generator::EncryptionKey;

pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub encryption_key: EncryptionKey,
}
