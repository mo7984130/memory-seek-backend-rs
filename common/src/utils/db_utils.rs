use crate::{error::AppError, ext::log_err};
use futures::future::BoxFuture;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionError, TransactionTrait};

pub struct DbUtils;

impl DbUtils {
    /// 在数据库事务中执行写操作
    ///
    /// 将闭包内的所有数据库操作包装在单个事务中，确保原子性。
    /// 连接错误统一转换为 `InternalServerError`。
    ///
    /// # 参数
    /// - `db`: 数据库连接
    /// - `block`: 接收事务引用的异步闭包，返回操作结果
    ///
    /// # 返回
    /// 返回闭包的执行结果
    ///
    /// # 错误
    /// - `AppError::InternalServerError`: 数据库连接错误
    /// - `AppError`: 闭包返回的业务错误（事务自动回滚）
    #[inline]
    pub async fn write<F, T>(db: &DatabaseConnection, block: F) -> Result<T, AppError>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> BoxFuture<'a, Result<T, AppError>> + Send,
        T: Send,
    {
        db.transaction(|txn| block(txn)).await.map_err(|e| match e {
            TransactionError::Connection(e) => log_err(
                "db_conn_err",
                "获取数据库连接错误",
                e,
                AppError::InternalServerError,
            ),
            TransactionError::Transaction(e) => e,
        })
    }
}
