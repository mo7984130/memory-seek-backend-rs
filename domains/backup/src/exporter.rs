use crate::hasher::TableHasher;
use csv::Writer;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// CSV 导出器
pub struct CsvExporter;

impl CsvExporter {
    /// 导出指定表到指定路径的 CSV 文件
    ///
    /// 返回 (文件路径, 数据哈希)
    pub async fn export_to_path(
        db: &DatabaseConnection,
        table_name: &str,
        output_path: &Path,
    ) -> Result<(PathBuf, String), Box<dyn std::error::Error + Send + Sync>> {
        let columns = TableHasher::get_column_names(db, table_name).await?;
        if columns.is_empty() {
            return Err(format!("Table {} does not exist", table_name).into());
        }
        let pks = TableHasher::get_primary_key_columns(db, table_name).await?;
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

        let mut wtr = Writer::from_path(output_path)?;

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

        let file_content = std::fs::read(output_path)?;
        let hash = compute_hash(&file_content);

        Ok((output_path.to_path_buf(), hash))
    }

    /// 导出指定表为 CSV，存到 output_dir/{table_name}.csv
    ///
    /// 返回 (文件路径, 数据哈希)
    pub async fn export(
        db: &DatabaseConnection,
        table_name: &str,
        output_dir: &Path,
    ) -> Result<(PathBuf, String), Box<dyn std::error::Error + Send + Sync>> {
        let output_path = output_dir.join(format!("{}.csv", table_name));
        Self::export_to_path(db, table_name, &output_path).await
    }
}

/// 计算数据的 SHA256 哈希
pub fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let data = b"test data";
        let hash1 = compute_hash(data);
        let hash2 = compute_hash(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_hash_different_data() {
        let data1 = b"data1";
        let data2 = b"data2";
        let hash1 = compute_hash(data1);
        let hash2 = compute_hash(data2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_hash_empty() {
        let data = b"";
        let hash = compute_hash(data);
        assert!(!hash.is_empty());
        // SHA256 of empty string is a well-known constant
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_compute_hash_returns_hex_string() {
        let hash = compute_hash(b"hello");
        // SHA256 produces 32 bytes = 64 hex characters
        assert_eq!(hash.len(), 64);
        // All characters should be valid hex digits
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
