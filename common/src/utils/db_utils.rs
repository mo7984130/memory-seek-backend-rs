use crate::Result;
use crate::ext::ToOk;
use crate::{error::AppError, ext::log_err_with_err};
use futures::future::BoxFuture;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, TransactionError, TransactionTrait,
};

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
    pub async fn write<F, T>(db: &DatabaseConnection, block: F) -> Result<T>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> BoxFuture<'a, Result<T>> + Send,
        T: Send,
    {
        db.transaction(|txn| block(txn)).await.map_err(|e| match e {
            TransactionError::Connection(e) => log_err_with_err(
                "db_conn_err",
                "获取数据库连接错误",
                e,
                AppError::InternalServerError,
            ),
            TransactionError::Transaction(e) => e,
        })
    }

    pub async fn lock_two_ordered<'a, DB, Id, T, F, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Id,
        f: F,
    ) -> Result<(Option<T>, Option<T>)>
    where
        DB: ConnectionTrait + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        Fut: Future<Output = Result<Option<T>>>,
    {
        let (first_id, second_id, reverse) = if id1 <= id2 {
            (id1, id2, false)
        } else {
            (id2, id1, true)
        };

        let first = f(db, first_id).await?;
        let second = f(db, second_id).await?;

        Ok(if reverse {
            (second, first)
        } else {
            (first, second)
        })
    }

    pub async fn ensure_lock_two_ordered<'a, DB, Id, T, F, MF, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Id,
        f: F,
        miss_f: MF,
    ) -> Result<(T, T)>
    where
        DB: ConnectionTrait + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        MF: Fn(Option<T>) -> Result<T>,
        Fut: Future<Output = Result<Option<T>>>,
    {
        let (first, second) = Self::lock_two_ordered(db, id1, id2, f).await?;
        (miss_f(first)?, miss_f(second)?).to_ok()
    }
}
