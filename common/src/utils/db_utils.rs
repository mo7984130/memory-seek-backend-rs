use std::pin::Pin;

use crate::DbConn;
use crate::Result;
use crate::error::ContextualResult;
use crate::error::{AppError, ContextualError, contextual};
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionError, TransactionTrait};

pub struct DbUtils;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
impl DbUtils {
    /// 在下层执行事务，但把连接和事务错误延迟到 service 边界再记录。
    pub async fn write_contextual<F, T>(db: &DatabaseConnection, block: F) -> contextual::Result<T>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> BoxFuture<'a, contextual::Result<T>> + Send,
        T: Send,
    {
        db.transaction(|txn| block(txn))
            .await
            .map_err(|error| match error {
                TransactionError::Connection(error) => ContextualError::error(
                    "db_conn_err",
                    "获取数据库连接错误",
                    error,
                    AppError::InternalServerError,
                ),
                TransactionError::Transaction(error) => error,
            })
    }

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
    pub async fn write<F, T>(db: &DatabaseConnection, block: F) -> ContextualResult<T>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> BoxFuture<'a, Result<T>> + Send,
        T: Send,
    {
        db.transaction(|txn| block(txn)).await.map_err(|e| match e {
            TransactionError::Connection(e) => ContextualError::error(
                "db_conn_err",
                "获取数据库连接错误",
                e,
                AppError::InternalServerError,
            ),
            TransactionError::Transaction(e) => ContextualError::error(
                "db_transaction_err",
                "数据库事务错误",
                e,
                AppError::InternalServerError,
            ),
        })
    }

    pub async fn lock_two_ordered<'a, DB, Id, T, F, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Id,
        f: F,
    ) -> Result<(Option<T>, Option<T>)>
    where
        DB: DbConn + ?Sized,
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

    pub async fn ensure_lock_two_ordered<'a, DB, Id, T, F, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Id,
        f: F,
    ) -> Result<(T, T)>
    where
        DB: DbConn + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        Fut: Future<Output = Result<T>>,
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

    pub async fn lock_two_optional_ordered<'a, DB, Id, T, F, MF, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Option<Id>,
        f: F,
        miss_f: MF,
    ) -> Result<(T, Option<T>)>
    where
        DB: DbConn + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        MF: Fn(Option<T>) -> Result<T>,
        Fut: Future<Output = Result<Option<T>>>,
    {
        match id2 {
            Some(id2) => {
                let (first_id, second_id, reverse) = if id1 <= id2 {
                    (id1, id2, false)
                } else {
                    (id2, id1, true)
                };
                let first = miss_f(f(db, first_id).await?)?;
                let second = miss_f(f(db, second_id).await?)?;
                Ok(if reverse {
                    (second, Some(first))
                } else {
                    (first, Some(second))
                })
            }
            None => Ok((miss_f(f(db, id1).await?)?, None)),
        }
    }

    /// 按 id 升序加锁两个实体, 其中第二个可选
    ///
    /// 与 `ensure_lock_two_ordered` 语义一致, 但 `id2` 可为 `None`:
    /// 为 `Some` 时对两个 id 升序加锁并返回 `(T, T)`, 为 `None` 时仅加锁
    /// `id1` 并返回 `(T, None)`。返回值的顺序始终与传入的 `id1`/`id2` 对应。
    pub async fn ensure_lock_two_optional_ordered<'a, DB, Id, T, F, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Option<Id>,
        f: F,
    ) -> Result<(T, Option<T>)>
    where
        DB: DbConn + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        match id2 {
            Some(id2) => {
                let (first_id, second_id, reverse) = if id1 <= id2 {
                    (id1, id2, false)
                } else {
                    (id2, id1, true)
                };
                let first = f(db, first_id).await?;
                let second = f(db, second_id).await?;
                Ok(if reverse {
                    (second, Some(first))
                } else {
                    (first, Some(second))
                })
            }
            None => Ok((f(db, id1).await?, None)),
        }
    }
}
