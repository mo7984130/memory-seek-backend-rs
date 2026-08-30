use crate::DbConn;
use sea_orm::Statement;

/// 数据库表元数据查询器
pub struct TableMetadata;

impl TableMetadata {
    /// 获取表的主键列名（按 ordinal_position 排序）
    pub async fn get_primary_key_columns(
        db: &impl DbConn,
        table_name: &str,
    ) -> Result<Vec<String>, sea_orm::DbErr> {
        let sql = format!(
            r#"
            SELECT c.column_name
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage c
              ON c.table_schema = tc.table_schema
              AND c.table_name = tc.table_name
              AND c.constraint_name = tc.constraint_name
            WHERE tc.constraint_type = 'PRIMARY KEY'
              AND tc.table_schema = 'public'
              AND tc.table_name = '{}'
            ORDER BY c.ordinal_position
        "#,
            table_name
        );
        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);

        let result = db.query_all_raw(stmt).await?;
        let mut columns = Vec::new();
        for row in &result {
            if let Ok(name) = row.try_get_by::<String, _>("column_name") {
                columns.push(name);
            }
        }
        Ok(columns)
    }

    /// 获取表的所有列名（按 ordinal_position 排序）
    pub async fn get_column_names(
        db: &impl DbConn,
        table_name: &str,
    ) -> Result<Vec<String>, sea_orm::DbErr> {
        let sql = format!(
            r#"
            SELECT column_name
            FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = '{}'
            ORDER BY ordinal_position
        "#,
            table_name
        );
        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        let result = db.query_all_raw(stmt).await?;
        let mut columns = Vec::new();
        for row in &result {
            if let Ok(name) = row.try_get_by::<String, _>("column_name") {
                columns.push(name);
            }
        }
        Ok(columns)
    }

    /// 获取所有用户表名
    pub async fn get_all_tables(db: &impl DbConn) -> Result<Vec<String>, sea_orm::DbErr> {
        let sql = r#"
            SELECT table_name
            FROM information_schema.tables
            WHERE table_schema = 'public'
              AND table_type = 'BASE TABLE'
            ORDER BY table_name
        "#;
        let stmt = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql.to_string());

        let result = db.query_all_raw(stmt).await?;
        let mut tables = Vec::new();

        for row in &result {
            if let Ok(name) = row.try_get_by::<String, _>("table_name") {
                tables.push(name);
            }
        }

        Ok(tables)
    }

    /// 获取 PostgreSQL 主版本，用于二进制 COPY 归档兼容性校验。
    pub async fn postgres_major_version(db: &impl DbConn) -> Result<u32, sea_orm::DbErr> {
        let stmt = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SHOW server_version_num".to_string(),
        );
        let row = db
            .query_one_raw(stmt)
            .await?
            .ok_or_else(|| sea_orm::DbErr::Custom("无法读取 PostgreSQL 版本".to_string()))?;
        let version = row
            .try_get_by::<String, _>("server_version_num")
            .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?;
        version
            .parse::<u32>()
            .map(|version| version / 10_000)
            .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))
    }
}
