use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use sha2::{Digest, Sha256};

/// 表级哈希计算器
pub struct TableHasher;

impl TableHasher {
    /// 计算整个表的 SHA256 哈希
    ///
    /// 使用 SELECT * ORDER BY id 查询所有数据，逐行计算哈希
    pub async fn compute(
        db: &DatabaseConnection,
        table_name: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let sql = format!("SELECT * FROM \"{}\" ORDER BY id", table_name);
        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);

        let result = db.query_all(stmt).await?;
        let mut hasher = Sha256::new();

        for row in &result {
            let cols = row.column_names();
            for col in cols {
                let value = row.try_get_by::<String, _>(col.as_str()).unwrap_or_default();
                hasher.update(value.as_bytes());
                hasher.update(b"|"); // 分隔符
            }
            hasher.update(b"\n"); // 行分隔符
        }

        Ok(hex::encode(hasher.finalize()))
    }

    /// 获取所有用户表名
    pub async fn get_all_tables(
        db: &DatabaseConnection,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let sql = r#"
            SELECT table_name
            FROM information_schema.tables
            WHERE table_schema = 'public'
              AND table_type = 'BASE TABLE'
            ORDER BY table_name
        "#;
        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql.to_string());

        let result = db.query_all(stmt).await?;
        let mut tables = Vec::new();

        for row in &result {
            if let Ok(name) = row.try_get_by::<String, _>("table_name") {
                tables.push(name);
            }
        }

        Ok(tables)
    }
}
