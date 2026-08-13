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
///     .await
/// }
/// ```
#[macro_export]
macro_rules! db_transaction {
    (scoped $db:expr, |$txn:ident| $body:block) => {
        async {
            let transaction = ::sea_orm::TransactionTrait::begin($db).await?;
            let result: $crate::Result<_> = async {
                let $txn = &transaction;
                $body
            }
            .await;

            match result {
                Ok(value) => {
                    transaction.commit().await?;
                    Ok(value)
                }
                Err(error) => {
                    transaction.rollback().await?;
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
