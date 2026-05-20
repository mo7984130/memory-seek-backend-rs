// db_error_ext.rs

use sea_orm::DbErr;

pub trait DbErrExt {
    fn is_unique_violation(&self) -> bool;
}

impl DbErrExt for DbErr {
    fn is_unique_violation(&self) -> bool {
        let msg = self.to_string();
        msg.contains("duplicate key value")         // PostgreSQL
        || msg.contains("Duplicate entry")          // MySQL
        || msg.contains("UNIQUE constraint failed") // SQLite
    }
}
