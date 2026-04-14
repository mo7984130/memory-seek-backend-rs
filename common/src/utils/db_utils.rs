use crate::error::AppError;
use futures::future::BoxFuture;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionError, TransactionTrait};

pub struct DbUtils;

impl DbUtils {
    #[inline]
    pub async fn write<F, T>(db: &DatabaseConnection, block: F) -> Result<T, AppError>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> BoxFuture<'a, Result<T, AppError>> + Send,
        T: Send,
    {
        db.transaction(|txn| {
            block(txn)
        })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(e) => {
                    tracing::error!("数据库连接错误: {}", e);
                    AppError::InternalServerError
                }
                TransactionError::Transaction(e) => e,
            })
    }
}