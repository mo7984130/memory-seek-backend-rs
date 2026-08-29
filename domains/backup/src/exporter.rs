use crate::error::BackupError;
use crate::error::Result;
use async_compression::tokio::write::ZstdEncoder;
use futures_util::TryStreamExt;
use sea_orm::DatabaseConnection;
use sqlx::postgres::PgPoolCopyExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// PostgreSQL 二进制 COPY 导出器。
pub struct BinaryCopyExporter;

impl BinaryCopyExporter {
    /// 将指定表流式导出为经 Zstd 压缩的 PostgreSQL 二进制 COPY 文件。
    pub async fn export_to_dir(
        db: &DatabaseConnection,
        table_name: &str,
        output_dir: &Path,
    ) -> Result<PathBuf> {
        let output_path = output_dir.join(format!("{table_name}.copy.zst"));

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let result = async {
            let copy_sql = format!(
                "COPY {} TO STDOUT (FORMAT binary)",
                Self::quote_identifier(table_name)
            );
            let mut source = db
                .get_postgres_connection_pool()
                .copy_out_raw(&copy_sql)
                .await?;
            let file = tokio::fs::File::create(&output_path).await?;
            let mut encoder = ZstdEncoder::new(file);

            while let Some(chunk) = source.try_next().await? {
                encoder.write_all(&chunk).await?;
            }
            encoder.shutdown().await?;
            Ok::<(), BackupError>(())
        }
        .await;

        if result.is_err() {
            let _ = tokio::fs::remove_file(&output_path).await;
        }
        result.map(|()| output_path)
    }

    fn quote_identifier(identifier: &str) -> String {
        format!("\"{}\"", identifier.replace('"', "\"\""))
    }
}

#[cfg(test)]
mod tests {
    use super::BinaryCopyExporter;

    #[test]
    fn quotes_table_identifier() {
        assert_eq!(
            BinaryCopyExporter::quote_identifier("photo\"archive"),
            "\"photo\"\"archive\""
        );
    }
}
