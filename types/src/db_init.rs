//! 数据库初始化:schema 同步与幂等索引初始化。
//!
//! 表结构同步由各实体 crate **自我登记**的 schema 前缀驱动
//! (见 [`types_db_api::SCHEMA_PREFIXES`]):sea-orm 的实体注册表按
//! `module_path!()` 字符串前缀匹配,且 `sync` 只增不删,因此按前缀逐个同步是安全的。

use common::{ContextualError, ContextualResult};
use sea_orm::DatabaseConnection;

pub use types_db_api::{INIT_INDEXES, InitIndexFn, InitIndexFuture, SCHEMA_PREFIXES};

/// 初始化数据库:同步表结构 + 执行幂等索引初始化回调。
pub async fn init_db(db: &DatabaseConnection) -> ContextualResult<()> {
    // 初始化表结构
    let mut synced = false;
    for prefix in SCHEMA_PREFIXES {
        synced = true;
        db.get_schema_registry(prefix.0)
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
    }
    if !synced {
        // 没有实体 crate 登记前缀说明链接图里没有实体(或忘记登记), 表结构不会被同步。
        tracing::warn!("没有实体 crate 登记 schema 前缀, 表结构不会被同步");
    }

    // 幂等初始化索引
    for init_index in INIT_INDEXES {
        init_index(db).await?;
    }

    Ok(())
}
