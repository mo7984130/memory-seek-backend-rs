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
#[macro_export]
macro_rules! db_transaction {
    ($db:expr, |$txn:ident| $body:block) => {
        $crate::utils::DbUtils::write($db, move |$txn| {
            ::std::boxed::Box::pin(async move $body)
        })
    };
}
