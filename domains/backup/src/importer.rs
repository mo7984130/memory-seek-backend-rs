use crate::error::Result;
use async_compression::tokio::bufread::ZstdDecoder;
use sea_orm::DatabaseConnection;
use sqlx::{Acquire, QueryBuilder};
use tokio::io::BufReader;

pub struct BinaryCopyImporter;

impl BinaryCopyImporter {
    pub async fn restore_local(
        db: &DatabaseConnection,
        tables: &[String],
        path_for: impl Fn(&str) -> std::path::PathBuf,
    ) -> Result<u64> {
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
            rows += copy.finish().await?;
        }
        transaction.commit().await?;
        Ok(rows)
    }

    fn quote_identifier(value: &str) -> String {
        format!("\"{}\"", value.replace('"', "\"\""))
    }
}
