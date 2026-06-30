use serde::Deserialize;

/// Backup module configuration
#[derive(Debug, Clone, Deserialize)]
pub struct BackupConfig {
    /// Enable backup feature
    #[serde(default)]
    pub enabled: bool,
    /// Cron schedule expression (e.g., "0 2 * * *" for daily at 2am)
    #[serde(default = "default_schedule")]
    pub schedule: String,
    /// Retention period in days
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// S3 bucket name for backups
    #[serde(default)]
    pub bucket: String,
    /// S3 prefix for backup files
    #[serde(default = "default_prefix")]
    pub prefix: String,
}

fn default_schedule() -> String {
    "0 2 * * *".to_string()
}

fn default_retention_days() -> u32 {
    30
}

fn default_prefix() -> String {
    "backups/".to_string()
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schedule: default_schedule(),
            retention_days: default_retention_days(),
            bucket: String::new(),
            prefix: default_prefix(),
        }
    }
}
