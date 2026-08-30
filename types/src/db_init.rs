use std::pin::Pin;

use common::{ContextualError, ContextualResult, DbConn};
use sea_orm::{DatabaseConnection, Statement};

pub type InitIndexFuture<'a> = Pin<Box<dyn Future<Output = ContextualResult<()>> + Send + 'a>>;

pub type InitIndexFn = for<'a> fn(&'a DatabaseConnection) -> InitIndexFuture<'a>;

#[linkme::distributed_slice]
pub static INIT_INDEXES: [InitIndexFn] = [..];

pub async fn init_db(db: &DatabaseConnection) -> ContextualResult<()> {
    // 开启向量拓展
    // 人脸需要
    #[cfg(feature = "face-engine")]
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        "CREATE EXTENSION IF NOT EXISTS vector;",
    ))
    .await?;

    // 初始化表结构
    db.get_schema_registry("types::*")
        .sync(db)
        .await
        .map_err(|source| {
            ContextualError::error(
                "db_sync_err",
                "数据库同步失败",
                source,
                common::error::AppError::InternalServerError,
            )
        })?;

    // 幂等初始化索引
    for init_index in INIT_INDEXES {
        init_index(db).await?;
    }

    Ok(())
}
