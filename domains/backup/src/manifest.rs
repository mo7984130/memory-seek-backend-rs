use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: &str = "postgres-copy-binary-zstd-v1";
pub const FILE_NAME: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub format_version: String,
    pub run_id: String,
    pub tables: Vec<String>,
    pub postgres_major: u32,
}

impl BackupManifest {
    pub fn new(run_id: String, tables: Vec<String>, postgres_major: u32) -> Self {
        Self {
            format_version: FORMAT_VERSION.to_string(),
            run_id,
            tables,
            postgres_major,
        }
    }
}
