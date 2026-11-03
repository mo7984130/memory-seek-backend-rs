use crate::error::{BackupError, Result};
use async_compression::tokio::bufread::ZstdDecoder;
use sea_orm::DatabaseConnection;
use sqlx::{Acquire, QueryBuilder};
use tokio::io::BufReader;
use tokio_util::sync::CancellationToken;

pub struct BinaryCopyImporter;

impl BinaryCopyImporter {
    /// 在单个事务内清空目标表并导入本地 COPY 文件，返回导入行数；失败或被取消时事务回滚
    pub async fn restore_local(
        db: &DatabaseConnection,
        tables: &[String],
        path_for: impl Fn(&str) -> std::path::PathBuf,
        cancellation: &CancellationToken,
    ) -> Result<u64> {
        // 数据层计时：`backup:restore:restore_local`。
        #[cfg(feature = "metrics")]
        let _timer =
            common_metrics::MetricsTimer::start(common_metrics::metrics_name!("restore_local"));

        Self::ensure_not_cancelled(cancellation)?;

        let mut connection = db.get_postgres_connection_pool().acquire().await?;

        let mut transaction = connection.begin().await?;

        let names = tables
            .iter()
            .map(|table| Self::quote_identifier(table))
            .collect::<Vec<_>>()
            .join(", ");

        let mut query = QueryBuilder::<sqlx::Postgres>::new("TRUNCATE TABLE ");

        query.push(&names);
        query.push(" RESTART IDENTITY");

        query.build().execute(&mut *transaction).await?;

        let mut rows = 0;
        for table in tables {
            Self::ensure_not_cancelled(cancellation)?;
            let file = tokio::fs::File::open(path_for(table)).await?;
            let mut copy = transaction
                .as_mut()
                .copy_in_raw(&format!(
                    "COPY {} FROM STDIN (FORMAT binary)",
                    Self::quote_identifier(table)
                ))
                .await?;
            copy.read_from(ZstdDecoder::new(BufReader::new(file)))
                .await?;
            let table_rows = copy.finish().await?;
            rows += table_rows;
            tracing::info!(table = %table, rows = table_rows, "已恢复表数据");
        }
        // 提交前最后确认一次，避免任务已被放弃或回收后仍写入数据。
        Self::ensure_not_cancelled(cancellation)?;
        transaction.commit().await?;
        Ok(rows)
    }

    fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<()> {
        if cancellation.is_cancelled() {
            return Err(BackupError::Msg("任务已取消".to_owned()));
        }
        Ok(())
    }

    /// 用双引号包裹标识符并转义内部双引号
    fn quote_identifier(value: &str) -> String {
        format!("\"{}\"", value.replace('"', "\"\""))
    }
}
