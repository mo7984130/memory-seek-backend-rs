/// 在数据库事务中执行异步代码，隐藏事务回调所需的 Future 装箱样板。
///
/// 事务成功时自动提交，返回错误时自动回滚。
///
/// # 示例
/// ```no_run
/// use common::{Result, db_transaction};
/// use sea_orm::DatabaseConnection;
///
/// async fn update(db: &DatabaseConnection) -> Result<()> {
///     db_transaction!(db, |txn| {
///         let _ = txn;
///         Ok(())
///     })
///     .await
/// }
/// ```
///
/// 当事务体需要借用调用方的局部变量时，使用 `scoped` 形式：
///
/// ```no_run
/// use common::{Result, db_transaction};
/// use sea_orm::DatabaseConnection;
///
/// async fn update(db: &DatabaseConnection, value: &mut u64) -> Result<()> {
///     db_transaction!(scoped db, |txn| {
///         let _ = txn;
///         *value += 1;
///         Ok(())
///     })
///     .await?;
///
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! db_transaction {
    (contextual $db:expr, |$txn:ident| $body:block) => {
        $crate::utils::DbUtils::write_contextual($db, move |$txn| {
            ::std::boxed::Box::pin(async move $body)
        })
    };
    (scoped $db:expr, |$txn:ident| $body:block) => {
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
    ($db:expr, |$txn:ident| $body:block) => {
        $crate::utils::DbUtils::write($db, move |$txn| {
            ::std::boxed::Box::pin(async move $body)
        })
    };
}
