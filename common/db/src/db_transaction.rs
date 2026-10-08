/// 在数据库事务中执行异步代码，统一 begin / commit / rollback 与错误映射，隐藏样板。
///
/// 事务成功时自动提交，返回错误时自动回滚。事务体返回 [`ContextualError`]，并可直接借用
/// 调用方的局部变量。
///
/// # 示例
/// ```no_run
/// use common::{ContextualResult, db_transaction};
/// use sea_orm::DatabaseConnection;
///
/// async fn update(db: &DatabaseConnection, value: &mut u64) -> ContextualResult<()> {
///     db_transaction!(db, |txn| {
///         let _ = txn;
///         *value += 1;
///         Ok(())
///     })
///     .await
/// }
/// ```
///
/// `contextual` 形式用于事务体已携带 `ContextualError`（延迟到 service 边界记录）的场景：
/// 事务体错误原样透传，不再包装为 `db_transaction_err`。
///
/// ```no_run
/// use common::{ContextualResult, db_transaction};
/// use sea_orm::DatabaseConnection;
///
/// async fn update(db: &DatabaseConnection) -> ContextualResult<()> {
///     db_transaction!(contextual db, |txn| {
///         let _ = txn;
///         Ok(())
///     })
///     .await
/// }
/// ```
#[macro_export]
macro_rules! db_transaction {
    (contextual $db:expr, |$txn:ident| $body:block) => {
        $crate::utils::DbUtils::write_contextual($db, move |$txn| {
            ::std::boxed::Box::pin(async move $body)
        })
    };
    ($db:expr, |$txn:ident| $body:block) => {
        async {
            let transaction = ::sea_orm::TransactionTrait::begin($db)
                .await
                .map_err(|error| {
                    $crate::error::ContextualError::error(
                        "db_conn_err",
                        "开启数据库事务失败",
                        error,
                        $crate::error::AppError::InternalServerError,
                    )
                })?;
            let result: $crate::error::contextual::Result<_> = async {
                let $txn = &transaction;
                $body
            }
            .await;

            match result {
                Ok(value) => {
                    transaction
                        .commit()
                        .await
                        .map_err(|error| {
                            $crate::error::ContextualError::error(
                                "db_commit_err",
                                "提交数据库事务失败",
                                error,
                                $crate::error::AppError::InternalServerError,
                            )
                        })?;
                    Ok(value)
                }
                Err(error) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(|error| {
                            $crate::error::ContextualError::error(
                                "db_rollback_err",
                                "回滚数据库事务失败",
                                error,
                                $crate::error::AppError::InternalServerError,
                            )
                        })?;
                    Err(error)
                }
            }
        }
    };
}
