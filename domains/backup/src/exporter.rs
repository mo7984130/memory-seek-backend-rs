use crate::error::BackupError;
use crate::error::Result;
use common::{DbConn, utils::table_metadata::TableMetadata};
use csv::Writer;
use sea_orm::Statement;
use std::path::{Path, PathBuf};

/// CSV 导出器
pub struct CsvExporter;

impl CsvExporter {
    /// 导出指定表到指定路径的 CSV 文件
    ///
    /// 返回导出的文件路径
    pub async fn export_to_dir(
        db: &impl DbConn,
        table_name: &str,
        output_dir: &Path,
    ) -> Result<PathBuf> {
        let output_path = output_dir.join(format!("{}.csv", table_name));

        let columns = TableMetadata::get_column_names(db, table_name).await?;
        if columns.is_empty() {
            return Err(BackupError::TableNotExist(table_name.to_string()));
        }
        let pks = TableMetadata::get_primary_key_columns(db, table_name).await?;
        let select_cols = columns
            .iter()
            .map(|c| format!("\"{}\"::text as \"{}\"", c, c))
            .collect::<Vec<_>>()
            .join(", ");
        let order_by = pks
            .iter()
            .map(|c| format!("\"{}\"", c))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {} FROM \"{}\" ORDER BY {}",
            select_cols, table_name, order_by
        );
        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);

        let result = db.query_all(stmt).await?;

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut wtr = Writer::from_path(&output_path)?;

        wtr.write_record(&columns)?;

        for row in &result {
            let mut record = Vec::new();
            for col in &columns {
                let value = row
                    .try_get_by::<String, _>(col.as_str())
                    .unwrap_or_default();
                record.push(value);
            }
            wtr.write_record(&record)?;
        }

        wtr.flush()?;

        Ok(output_path)
    }
}
