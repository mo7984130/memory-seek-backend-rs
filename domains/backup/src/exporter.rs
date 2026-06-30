use csv::Writer;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// CSV 导出器
pub struct CsvExporter;

impl CsvExporter {
    /// 导出指定表为 CSV
    ///
    /// 返回 (文件路径, 数据哈希)
    pub async fn export(
        db: &DatabaseConnection,
        table_name: &str,
        output_dir: &Path,
    ) -> Result<(PathBuf, String), Box<dyn std::error::Error + Send + Sync>> {
        let sql = format!("SELECT * FROM \"{}\" ORDER BY id", table_name);
        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);

        let result = db.query_all(stmt).await?;

        // 获取列名
        let columns = if let Some(first_row) = result.first() {
            first_row.column_names()
        } else {
            return Err(format!("Table {} is empty or does not exist", table_name).into());
        };

        // 创建 CSV 文件
        let file_path = output_dir.join(format!("{}.csv", table_name));
        let mut wtr = Writer::from_path(&file_path)?;

        // 写入表头
        wtr.write_record(&columns)?;

        // 写入数据行
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

        // 计算文件哈希
        let file_content = std::fs::read(&file_path)?;
        let hash = compute_hash(&file_content);

        Ok((file_path, hash))
    }
}

/// 计算数据的 SHA256 哈希
pub fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
