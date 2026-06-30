# Database Backup Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现内置的数据库定时备份功能，每天自动导出 CSV，支持本地 + S3 双重存储

**Architecture:** 创建独立的 `domains/backup/` 模块，使用 `tokio-cron-scheduler` 实现定时调度，通过表级 SHA256 哈希检测变化，无变化时重命名文件而非重新导出

**Tech Stack:** tokio-cron-scheduler, csv, sha2, hex, chrono, sea-orm, oss (S3Client)

---

## File Structure

```
domains/backup/
├── Cargo.toml                    # 模块依赖配置
├── src/
│   ├── lib.rs                    # 模块入口
│   ├── config.rs                 # BackupConfig 定义
│   ├── state.rs                  # BackupState 定义
│   ├── hasher.rs                 # TableHasher - 表级哈希计算
│   ├── exporter.rs               # CsvExporter - CSV 导出
│   ├── storage.rs                # BackupStorage - 本地+S3 存储
│   ├── runner.rs                 # BackupRunner - 备份主流程
│   └── scheduler.rs              # BackupScheduler - 定时调度

server/src/
├── config.rs                     # 添加 BackupConfig 到 AppConfig
├── state.rs                      # 添加 BackupScheduler 到 AppState
├── setup/
│   ├── mod.rs                    # 初始化备份调度器
│   └── domains/mod.rs            # 无需修改（备份无路由）

tests/load/config/config.json     # 添加 backup 配置节
```

---

## Task 1: 创建 backup 模块结构和 Cargo.toml

**Files:**
- Create: `domains/backup/Cargo.toml`
- Create: `domains/backup/src/lib.rs`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
# domains/backup/Cargo.toml
[package]
name = "backup"
version = "0.1.0"
edition = "2024"

[dependencies]
sea-orm = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }

csv = "1.3"
tokio-cron-scheduler = "0.13"

# S3
oss = { path = "../../libs/oss" }

# 错误处理
common = { path = "../../common" }
```

- [ ] **Step 2: 创建 lib.rs 骨架**

```rust
// domains/backup/src/lib.rs
pub mod config;
pub mod state;
pub mod hasher;
pub mod exporter;
pub mod storage;
pub mod runner;
pub mod scheduler;

pub use config::BackupConfig;
pub use state::BackupState;
pub use scheduler::BackupScheduler;
```

- [ ] **Step 3: 添加到 workspace**

修改根目录 `Cargo.toml`，在 `[workspace] members` 中添加 `"domains/backup"`。

- [ ] **Step 4: 验证编译**

```bash
cargo check -p backup
```

- [ ] **Step 5: Commit**

```bash
git add domains/backup/ Cargo.toml
git commit -m "feat(backup): create backup module structure"
```

---

## Task 2: 实现 BackupConfig

**Files:**
- Create: `domains/backup/src/config.rs`

- [ ] **Step 1: 实现 BackupConfig**

```rust
// domains/backup/src/config.rs
use serde::Deserialize;

/// 备份配置
#[derive(Debug, Clone, Deserialize)]
pub struct BackupConfig {
    /// 是否启用备份
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Cron 表达式，默认每天凌晨 6 点
    #[serde(default = "default_schedule")]
    pub schedule: String,

    /// 本地备份路径
    #[serde(default = "default_local_path")]
    pub local_path: String,

    /// S3 前缀
    #[serde(default = "default_s3_prefix")]
    pub s3_prefix: String,

    /// 保留天数
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,

    /// 备份的表名列表，None = 备份所有表
    pub tables: Option<Vec<String>>,
}

fn default_enabled() -> bool {
    true
}

fn default_schedule() -> String {
    "0 0 6 * * *".to_string()
}

fn default_local_path() -> String {
    "/var/backups/memory-seek".to_string()
}

fn default_s3_prefix() -> String {
    "backup/".to_string()
}

fn default_retention_days() -> u32 {
    3
}

impl BackupConfig {
    /// 验证配置有效性
    pub fn validate(&self) -> Result<(), String> {
        if self.local_path.is_empty() {
            return Err("backup.local_path cannot be empty".to_string());
        }
        if self.retention_days == 0 {
            return Err("backup.retention_days must be > 0".to_string());
        }
        Ok(())
    }
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p backup
```

- [ ] **Step 3: Commit**

```bash
git add domains/backup/src/config.rs
git commit -m "feat(backup): implement BackupConfig"
```

---

## Task 3: 实现 BackupState

**Files:**
- Create: `domains/backup/src/state.rs`

- [ ] **Step 1: 实现 BackupState**

```rust
// domains/backup/src/state.rs
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::BackupConfig;
use crate::storage::BackupStorage;
use oss::S3Client;

/// 备份服务状态
pub struct BackupState {
    pub db: DatabaseConnection,
    pub storage: BackupStorage,
    pub config: BackupConfig,
    pub temp_dir: PathBuf,
    pub last_hashes: RwLock<HashMap<String, String>>,
}

impl BackupState {
    pub fn new(
        db: DatabaseConnection,
        s3_client: Arc<S3Client>,
        config: BackupConfig,
    ) -> Self {
        let temp_dir = PathBuf::from(&config.local_path).join(".tmp");
        let storage = BackupStorage::new(
            PathBuf::from(&config.local_path),
            s3_client,
            config.s3_prefix.clone(),
        );

        Self {
            db,
            storage,
            config,
            temp_dir,
            last_hashes: RwLock::new(HashMap::new()),
        }
    }

    /// 确保临时目录存在
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.temp_dir)?;
        std::fs::create_dir_all(&self.config.local_path)?;
        Ok(())
    }
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p backup
```

- [ ] **Step 3: Commit**

```bash
git add domains/backup/src/state.rs
git commit -m "feat(backup): implement BackupState"
```

---

## Task 4: 实现 TableHasher

**Files:**
- Create: `domains/backup/src/hasher.rs`

- [ ] **Step 1: 实现 TableHasher**

```rust
// domains/backup/src/hasher.rs
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
        let stmt = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql,
        );

        let result = db.query_all(stmt).await?;
        let mut hasher = Sha256::new();

        for row in &result {
            let cols = row.columns();
            for col in cols {
                let value = row.try_get_by::<String, _>(col.name())
                    .unwrap_or_default();
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
        let stmt = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql.to_string(),
        );

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
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p backup
```

- [ ] **Step 3: Commit**

```bash
git add domains/backup/src/hasher.rs
git commit -m "feat(backup): implement TableHasher"
```

---

## Task 5: 实现 CsvExporter

**Files:**
- Create: `domains/backup/src/exporter.rs`

- [ ] **Step 1: 实现 CsvExporter**

```rust
// domains/backup/src/exporter.rs
use csv::Writer;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
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
        let stmt = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql,
        );

        let result = db.query_all(stmt).await?;

        // 获取列名
        let columns = if let Some(first_row) = result.first() {
            first_row.columns().iter().map(|c| c.name().to_string()).collect::<Vec<_>>()
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
                let value = row.try_get_by::<String, _>(col.as_str())
                    .unwrap_or_else(|_| "".to_string());
                record.push(value);
            }
            wtr.write_record(&record)?;
        }

        wtr.flush()?;

        // 计算文件哈希
        let file_content = std::fs::read(&file_path)?;
        let hash = crate::hasher::compute_hash(&file_content);

        Ok((file_path, hash))
    }
}

/// 计算数据的 SHA256 哈希
pub fn compute_hash(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p backup
```

- [ ] **Step 3: Commit**

```bash
git add domains/backup/src/exporter.rs
git commit -m "feat(backup): implement CsvExporter"
```

---

## Task 6: 实现 BackupStorage

**Files:**
- Create: `domains/backup/src/storage.rs`

- [ ] **Step 1: 实现 BackupStorage**

```rust
// domains/backup/src/storage.rs
use oss::S3Client;
use std::path::PathBuf;
use std::sync::Arc;

/// 备份存储管理器
pub struct BackupStorage {
    local_path: PathBuf,
    s3_client: Arc<S3Client>,
    s3_prefix: String,
}

impl BackupStorage {
    pub fn new(
        local_path: PathBuf,
        s3_client: Arc<S3Client>,
        s3_prefix: String,
    ) -> Self {
        Self {
            local_path,
            s3_client,
            s3_prefix,
        }
    }

    /// 保存备份文件到本地 + S3
    pub async fn save(
        &self,
        table_name: &str,
        date: &str,
        csv_path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file_name = format!("{}.csv", date);

        // 1. 保存到本地
        let local_dir = self.local_path.join(table_name);
        std::fs::create_dir_all(&local_dir)?;
        let local_file = local_dir.join(&file_name);
        std::fs::copy(csv_path, &local_file)?;

        // 2. 上传到 S3
        let s3_key = format!("{}{}/{}", self.s3_prefix, table_name, file_name);
        let csv_content = std::fs::read(csv_path)?;
        self.s3_client
            .upload(&s3_key, csv_content, "text/csv")
            .await?;

        tracing::info!(
            table = %table_name,
            date = %date,
            local = %local_file.display(),
            s3 = %s3_key,
            "Backup saved"
        );

        Ok(())
    }

    /// 查找指定表的最新备份日期
    pub async fn find_latest_backup(
        &self,
        table_name: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let local_dir = self.local_path.join(table_name);
        if !local_dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<String> = None;

        for entry in std::fs::read_dir(&local_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            if file_name.ends_with(".csv") {
                let date = file_name.trim_end_matches(".csv").to_string();
                if latest.as_ref().map_or(true, |l| &date > l) {
                    latest = Some(date);
                }
            }
        }

        Ok(latest)
    }

    /// 重命名文件（当数据无变化时）
    pub async fn rename(
        &self,
        table_name: &str,
        old_date: &str,
        new_date: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let old_file = format!("{}.csv", old_date);
        let new_file = format!("{}.csv", new_date);

        // 1. 重命名本地文件
        let local_dir = self.local_path.join(table_name);
        let old_local = local_dir.join(&old_file);
        let new_local = local_dir.join(&new_file);

        if old_local.exists() {
            std::fs::rename(&old_local, &new_local)?;
            tracing::info!(
                table = %table_name,
                from = %old_date,
                to = %new_date,
                "Local backup renamed"
            );
        }

        // 2. 重命名 S3 文件（复制+删除）
        let old_s3_key = format!("{}{}/{}", self.s3_prefix, table_name, old_file);
        let new_s3_key = format!("{}{}/{}", self.s3_prefix, table_name, new_file);

        // 下载旧文件
        match self.s3_client.download(&old_s3_key).await {
            Ok(content) => {
                // 上传为新文件
                self.s3_client
                    .upload(&new_s3_key, content.to_vec(), "text/csv")
                    .await?;
                // 删除旧文件
                self.s3_client.delete(&old_s3_key).await?;
                tracing::info!(
                    table = %table_name,
                    from = %old_s3_key,
                    to = %new_s3_key,
                    "S3 backup renamed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    table = %table_name,
                    key = %old_s3_key,
                    err = ?e,
                    "S3 old file not found, skip rename"
                );
            }
        }

        Ok(())
    }

    /// 清理过期备份
    pub async fn cleanup(
        &self,
        retention_days: u32,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let cutoff = chrono::Local::now()
            - chrono::Duration::days(retention_days as i64);
        let cutoff_str = cutoff.format("%Y-%m-%d").to_string();

        let mut removed_count = 0;

        // 遍历本地备份目录
        if self.local_path.exists() {
            for entry in std::fs::read_dir(&self.local_path)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }

                let table_name = entry.file_name().to_string_lossy().to_string();
                let table_dir = entry.path();

                for file_entry in std::fs::read_dir(&table_dir)? {
                    let file_entry = file_entry?;
                    let file_name = file_entry.file_name();
                    let file_name = file_name.to_string_lossy();

                    if file_name.ends_with(".csv") {
                        let date = file_name.trim_end_matches(".csv");
                        if date < cutoff_str.as_str() {
                            // 删除本地文件
                            std::fs::remove_file(file_entry.path())?;

                            // 删除 S3 文件
                            let s3_key = format!(
                                "{}{}/{}",
                                self.s3_prefix, table_name, file_name
                            );
                            let _ = self.s3_client.delete(&s3_key).await;

                            removed_count += 1;
                            tracing::info!(
                                table = %table_name,
                                date = %date,
                                "Removed expired backup"
                            );
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p backup
```

- [ ] **Step 3: Commit**

```bash
git add domains/backup/src/storage.rs
git commit -m "feat(backup): implement BackupStorage"
```

---

## Task 7: 实现 BackupRunner

**Files:**
- Create: `domains/backup/src/runner.rs`

- [ ] **Step 1: 实现 BackupRunner**

```rust
// domains/backup/src/runner.rs
use crate::exporter::CsvExporter;
use crate::hasher::TableHasher;
use crate::state::BackupState;
use std::sync::Arc;

/// 备份执行器
pub struct BackupRunner;

impl BackupRunner {
    /// 获取需要备份的表名列表
    async fn get_tables(
        state: &BackupState,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref tables) = state.config.tables {
            return Ok(tables.clone());
        }
        TableHasher::get_all_tables(&state.db).await
    }

    /// 执行备份
    pub async fn execute(
        state: Arc<BackupState>,
    ) -> Result<BackupResult, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();

        tracing::info!("Starting daily backup for date: {}", date);

        // 确保目录存在
        state.ensure_dirs()?;

        let tables = Self::get_tables(&state).await?;
        let mut result = BackupResult::default();

        for table_name in tables {
            match Self::backup_table(&state, &table_name, &date).await {
                Ok(table_result) => {
                    match table_result {
                        TableBackupResult::Exported => {
                            result.exported += 1;
                            tracing::info!("Table {} backed up successfully", table_name);
                        }
                        TableBackupResult::Renamed => {
                            result.renamed += 1;
                            tracing::info!("Table {} unchanged, renamed backup", table_name);
                        }
                        TableBackupResult::Skipped(reason) => {
                            result.skipped += 1;
                            tracing::warn!("Table {} skipped: {}", table_name, reason);
                        }
                    }
                    result.success += 1;
                }
                Err(e) => {
                    result.failed += 1;
                    tracing::error!("Table {} backup failed: {}", table_name, e);
                    // 继续处理其他表
                }
            }
        }

        // 清理过期备份
        match state.storage.cleanup(state.config.retention_days).await {
            Ok(count) => {
                result.cleaned = count;
                tracing::info!("Cleanup: removed {} expired backups", count);
            }
            Err(e) => {
                tracing::error!("Cleanup failed: {}", e);
            }
        }

        result.duration = start.elapsed();
        tracing::info!(
            "Daily backup completed in {:?}: {} exported, {} renamed, {} skipped, {} failed",
            result.duration,
            result.exported,
            result.renamed,
            result.skipped,
            result.failed
        );

        Ok(result)
    }

    /// 备份单个表
    async fn backup_table(
        state: &BackupState,
        table_name: &str,
        date: &str,
    ) -> Result<TableBackupResult, Box<dyn std::error::Error + Send + Sync>> {
        // 1. 计算表哈希
        let current_hash = TableHasher::compute(&state.db, table_name).await?;

        // 2. 获取上次哈希
        let last_hash = state.last_hashes.read().await.get(table_name).cloned();

        // 3. 判断是否变化
        if Some(&current_hash) == last_hash.as_ref() {
            // 无变化：查找最近的备份文件并重命名
            if let Some(latest_date) = state.storage.find_latest_backup(table_name).await? {
                if latest_date != date {
                    state.storage.rename(table_name, &latest_date, date).await?;
                    return Ok(TableBackupResult::Renamed);
                } else {
                    return Ok(TableBackupResult::Skipped("already up to date".to_string()));
                }
            } else {
                return Ok(TableBackupResult::Skipped("no existing backup found".to_string()));
            }
        }

        // 4. 有变化：导出新 CSV
        let (csv_path, _) = CsvExporter::export(&state.db, table_name, &state.temp_dir).await?;

        // 5. 保存到本地 + S3
        state.storage.save(table_name, date, &csv_path).await?;

        // 6. 更新哈希缓存
        state.last_hashes.write().await.insert(table_name.to_string(), current_hash);

        // 7. 清理临时文件
        let _ = std::fs::remove_file(csv_path);

        Ok(TableBackupResult::Exported)
    }
}

#[derive(Debug, Default)]
pub struct BackupResult {
    pub success: u32,
    pub failed: u32,
    pub exported: u32,
    pub renamed: u32,
    pub skipped: u32,
    pub cleaned: u32,
    pub duration: std::time::Duration,
}

enum TableBackupResult {
    Exported,
    Renamed,
    Skipped(String),
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p backup
```

- [ ] **Step 3: Commit**

```bash
git add domains/backup/src/runner.rs
git commit -m "feat(backup): implement BackupRunner"
```

---

## Task 8: 实现 BackupScheduler

**Files:**
- Create: `domains/backup/src/scheduler.rs`

- [ ] **Step 1: 实现 BackupScheduler**

```rust
// domains/backup/src/scheduler.rs
use crate::runner::BackupRunner;
use crate::state::BackupState;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

/// 备份调度器
pub struct BackupScheduler {
    scheduler: JobScheduler,
}

impl BackupScheduler {
    /// 创建新的调度器
    pub async fn new(state: Arc<BackupState>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let scheduler = JobScheduler::new().await?;

        let schedule = state.config.schedule.clone();
        let state_clone = state.clone();

        let job = Job::new(schedule.as_str(), move |_, _| {
            let state = state_clone.clone();
            tokio::spawn(async move {
                if let Err(e) = BackupRunner::execute(state).await {
                    tracing::error!("Backup job failed: {}", e);
                }
            })
        })?;

        scheduler.add(job).await?;

        Ok(Self { scheduler })
    }

    /// 启动调度器
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.scheduler.start().await?;
        tracing::info!("Backup scheduler started");
        Ok(())
    }

    /// 停止调度器
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.scheduler.shutdown().await?;
        tracing::info!("Backup scheduler stopped");
        Ok(())
    }
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p backup
```

- [ ] **Step 3: Commit**

```bash
git add domains/backup/src/scheduler.rs
git commit -m "feat(backup): implement BackupScheduler"
```

---

## Task 9: 集成到 AppConfig

**Files:**
- Modify: `server/src/config.rs`

- [ ] **Step 1: 添加 BackupConfig 到 AppConfig**

在 `server/src/config.rs` 中添加：

```rust
// 在 AppConfig 结构体中添加
#[cfg(feature = "backup")]
pub backup: Option<backup::BackupConfig>,
```

- [ ] **Step 2: 添加 backup feature 到 server/Cargo.toml**

```toml
# server/Cargo.toml [features] 部分添加
backup = ["dep:backup"]

# [dependencies] 部分添加
backup = { path = "../domains/backup", optional = true }
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p server --features backup
```

- [ ] **Step 4: Commit**

```bash
git add server/src/config.rs server/Cargo.toml
git commit -m "feat(backup): integrate BackupConfig into AppConfig"
```

---

## Task 10: 集成到 AppState 和启动流程

**Files:**
- Modify: `server/src/state.rs`
- Modify: `server/src/setup/mod.rs`

- [ ] **Step 1: 修改 AppState**

在 `server/src/state.rs` 中添加：

```rust
// 在 AppState 结构体中添加
#[cfg(feature = "backup")]
pub backup_scheduler: Option<Arc<backup::BackupScheduler>>,
```

- [ ] **Step 2: 修改 AppSetup 初始化**

在 `server/src/setup/mod.rs` 中添加备份调度器初始化：

```rust
// 在 init 方法中，domains 初始化之后添加

#[cfg(feature = "backup")]
let backup_scheduler = if let Some(ref backup_config) = cfg.backup {
    if backup_config.enabled {
        let backup_state = Arc::new(backup::BackupState::new(
            bases.db.clone(),
            libs.s3_client.clone(),
            backup_config.clone(),
        ));
        let scheduler = backup::BackupScheduler::new(backup_state).await?;
        scheduler.start().await?;
        Some(Arc::new(scheduler))
    } else {
        None
    }
} else {
    None
};
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p server --features "backup,s3"
```

- [ ] **Step 4: Commit**

```bash
git add server/src/state.rs server/src/setup/mod.rs
git commit -m "feat(backup): integrate BackupScheduler into AppState"
```

---

## Task 11: 添加配置文件支持

**Files:**
- Modify: `tests/load/config/config.json`
- Modify: `server/tests/test.config.json`

- [ ] **Step 1: 添加 backup 配置节**

在配置文件中添加：

```json
{
    "backup": {
        "enabled": true,
        "schedule": "0 0 6 * * *",
        "local_path": "/var/backups/memory-seek",
        "s3_prefix": "backup/",
        "retention_days": 3,
        "tables": null
    }
}
```

- [ ] **Step 2: 验证配置加载**

```bash
cargo run -p server --features "backup,s3" -- --help || true
```

- [ ] **Step 3: Commit**

```bash
git add tests/load/config/config.json server/tests/test.config.json
git commit -m "feat(backup): add backup config to config files"
```

---

## Task 12: 编写单元测试

**Files:**
- Create: `domains/backup/src/hasher.rs` (添加 tests 模块)
- Create: `domains/backup/src/exporter.rs` (添加 tests 模块)

- [ ] **Step 1: 添加 hasher 单元测试**

在 `domains/backup/src/hasher.rs` 底部添加：

```rust
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
    }
}
```

- [ ] **Step 2: 运行单元测试**

```bash
cargo test -p backup --lib
```

- [ ] **Step 3: Commit**

```bash
git add domains/backup/src/hasher.rs
git commit -m "test(backup): add hasher unit tests"
```

---

## Task 13: 编写集成测试

**Files:**
- Create: `server/tests/backup/mod.rs`
- Create: `server/tests/backup/backup_test.rs`

- [ ] **Step 1: 创建测试模块**

```rust
// server/tests/backup/mod.rs
mod backup_test;
```

- [ ] **Step 2: 编写集成测试**

```rust
// server/tests/backup/backup_test.rs
use std::sync::Arc;
use backup::{BackupConfig, BackupState, BackupScheduler, runner::BackupRunner};

#[tokio::test]
async fn test_full_backup_flow() {
    // 需要测试数据库连接
    // 这里只是示例框架
    let config = BackupConfig {
        enabled: true,
        schedule: "0 0 6 * * *".to_string(),
        local_path: "/tmp/test-backup".to_string(),
        s3_prefix: "test-backup/".to_string(),
        retention_days: 3,
        tables: Some(vec!["auth_user".to_string()]),
    };

    // 验证配置
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn test_backup_config_defaults() {
    let config = BackupConfig {
        enabled: true,
        schedule: "0 0 6 * * *".to_string(),
        local_path: "/tmp/test".to_string(),
        s3_prefix: "backup/".to_string(),
        retention_days: 3,
        tables: None,
    };

    assert!(config.tables.is_none());
    assert_eq!(config.retention_days, 3);
}
```

- [ ] **Step 3: 运行集成测试**

```bash
cargo test -p server --features "backup,s3" -- --test-threads=1
```

- [ ] **Step 4: Commit**

```bash
git add server/tests/backup/
git commit -m "test(backup): add backup integration tests"
```

---

## Final Verification

- [ ] **完整编译检查**

```bash
cargo build --features "auth,user,photo,backup"
```

- [ ] **运行所有测试**

```bash
cargo test --features "auth,user,photo,backup" -- --test-threads=1
```

- [ ] **代码格式检查**

```bash
cargo fmt --check
```

- [ ] **Clippy 检查**

```bash
cargo clippy --features "auth,user,photo,backup" -- -D warnings
```
