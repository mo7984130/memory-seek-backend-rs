# 数据库定时备份模块设计

## 概述

为 Memory Seek 后端服务添加内置的数据库定时备份功能，每天自动导出所有表的 CSV 数据，支持本地 + S3 双重存储，保留最近 3 天的备份。

## 需求

- **备份格式**: CSV 数据导出
- **变化检测**: 表级哈希比较（SHA256）
- **存储位置**: 本地磁盘 + S3 (memory-seek/backup/)
- **无变化处理**: 直接重命名文件（不重新导出）
- **执行时间**: 每天凌晨 6:00
- **保留策略**: 保留最近 3 天
- **备份范围**: 所有表（可通过配置指定特定表）

## 架构设计

### 模块结构

```
domains/backup/
├── Cargo.toml
├── src/
│   ├── lib.rs              # 模块入口，导出 AppModule
│   ├── state.rs            # BackupState 定义
│   ├── config.rs           # 备份配置结构
│   ├── scheduler.rs        # 定时调度器封装
│   ├── exporter.rs         # CSV 导出逻辑
│   ├── hasher.rs           # 表级哈希计算
│   ├── storage.rs          # 本地+S3 存储管理
│   └── cleanup.rs          # 过期备份清理
```

### Cargo 依赖

```toml
[dependencies]
tokio-cron-scheduler = "0.10"  # 定时调度
csv = "1.3"                    # CSV 写入
sha2 = "0.10"                  # SHA256 哈希
hex = "0.4"                    # 十六进制编码
chrono = "0.4"                 # 时间处理
tokio = { version = "1", features = ["full"] }
tracing = "0.1"                # 日志
```

## 核心组件

### BackupConfig

```rust
pub struct BackupConfig {
    pub enabled: bool,                    // 是否启用备份
    pub schedule: String,                 // Cron 表达式，默认 "0 0 6 * * *"
    pub local_path: String,               // 本地备份路径，如 "/var/backups/memory-seek"
    pub s3_prefix: String,                // S3 前缀，如 "backup/"
    pub retention_days: u32,              // 保留天数，默认 3
    pub tables: Option<Vec<String>>,      // None = 备份所有表
}
```

### BackupState

```rust
pub struct BackupState {
    pub db: DatabaseConnection,           // 数据库连接
    pub s3_client: Arc<oss::S3Client>,    // S3 客户端
    pub config: BackupConfig,             // 备份配置
    pub last_hashes: RwLock<HashMap<String, String>>,  // 表名 -> 上次哈希值
}
```

### BackupScheduler

```rust
pub struct BackupScheduler {
    scheduler: JobScheduler,
}

impl BackupScheduler {
    pub async fn new(state: Arc<BackupState>) -> Result<Self>;
    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self) -> Result<()>;
}
```

- 使用 `tokio-cron-scheduler` 创建定时任务
- 在 AppState 初始化时启动
- 服务关闭时优雅停止

### CsvExporter

```rust
pub struct CsvExporter;

impl CsvExporter {
    /// 导出指定表为 CSV，返回 (文件路径, 数据哈希)
    pub async fn export(
        db: &DatabaseConnection,
        table_name: &str,
        output_dir: &Path,
    ) -> Result<(PathBuf, String)>;

    /// 导出所有表
    pub async fn export_all(
        db: &DatabaseConnection,
        tables: &[String],
        output_dir: &Path,
    ) -> Result<Vec<(String, PathBuf, String)>>;  // (表名, 路径, 哈希)
}
```

### TableHasher

```rust
pub struct TableHasher;

impl TableHasher {
    /// 计算整个表的 SHA256 哈希
    /// 使用 SELECT * ORDER BY id，逐行计算哈希
    pub async fn compute(
        db: &DatabaseConnection,
        table_name: &str,
    ) -> Result<String>;
}
```

### BackupStorage

```rust
pub struct BackupStorage {
    local_path: PathBuf,
    s3_client: Arc<oss::S3Client>,
    s3_prefix: String,
}

impl BackupStorage {
    /// 保存备份文件到本地 + S3
    pub async fn save(
        &self,
        table_name: &str,
        date: &str,
        csv_path: &Path,
    ) -> Result<()>;

    /// 重命名文件（当数据无变化时）
    pub async fn rename(
        &self,
        table_name: &str,
        old_date: &str,
        new_date: &str,
    ) -> Result<()>;

    /// 清理过期备份
    pub async fn cleanup(&self, retention_days: u32) -> Result<()>;
}
```

## 备份流程

```rust
pub struct BackupRunner;

impl BackupRunner {
    pub async fn execute(state: Arc<BackupState>) -> Result<()> {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let tables = Self::get_tables(&state).await?;

        for table_name in tables {
            // 1. 计算表哈希
            let current_hash = TableHasher::compute(&state.db, &table_name).await?;

            // 2. 获取上次哈希
            let last_hash = state.last_hashes.read().await
                .get(&table_name).cloned();

            // 3. 判断是否变化
            if Some(&current_hash) == last_hash.as_ref() {
                // 无变化：重命名昨天的文件
                let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
                    .format("%Y-%m-%d").to_string();
                state.storage.rename(&table_name, &yesterday, &date).await?;
                tracing::info!("Table {} unchanged, renamed backup", table_name);
            } else {
                // 有变化：导出新 CSV
                let (csv_path, _) = CsvExporter::export(
                    &state.db, &table_name, &state.temp_dir
                ).await?;

                // 4. 保存到本地 + S3
                state.storage.save(&table_name, &date, &csv_path).await?;

                // 5. 更新哈希缓存
                state.last_hashes.write().await
                    .insert(table_name.clone(), current_hash);

                tracing::info!("Table {} backed up successfully", table_name);
            }
        }

        // 6. 清理过期备份
        state.storage.cleanup(state.config.retention_days).await?;

        Ok(())
    }
}
```

## 集成方式

### AppState 集成

```rust
pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub token_cipher: Arc<TokenCipher>,
    pub s3_client: Arc<oss::S3Client>,
    pub backup_state: Option<Arc<BackupState>>,  // 新增
}

// 初始化时启动备份调度
if let Some(ref backup_config) = cfg.backup {
    if backup_config.enabled {
        let backup_state = Arc::new(BackupState::new(
            db.clone(), s3_client.clone(), backup_config.clone()
        ));
        let scheduler = BackupScheduler::new(backup_state).await?;
        scheduler.start().await?;
        // 存储 scheduler 以便优雅关闭
    }
}
```

### 配置文件格式

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

### 文件命名规则

```
本地: /var/backups/memory-seek/{table_name}/{date}.csv
S3:   backup/{table_name}/{date}.csv

示例:
- /var/backups/memory-seek/auth_user/2026-06-30.csv
- backup/photo_photo/2026-06-30.csv
```

## 测试策略

### 单元测试

```rust
// hasher.rs
#[tokio::test]
async fn test_compute_hash_deterministic() {
    // 同表多次计算应得到相同哈希
}

#[tokio::test]
async fn test_compute_hash_changes_on_update() {
    // 数据变化后哈希应不同
}

// exporter.rs
#[tokio::test]
async fn test_export_csv_format() {
    // 验证 CSV 格式正确
}

#[tokio::test]
async fn test_export_empty_table() {
    // 空表导出应生成表头
}
```

### 集成测试

```rust
// server/tests/backup/
#[tokio::test]
async fn test_full_backup_flow() {
    // 完整备份流程测试
    // 1. 创建测试数据
    // 2. 执行备份
    // 3. 验证本地文件
    // 4. 验证 S3 文件
}

#[tokio::test]
async fn test_unchanged_table_rename() {
    // 测试无变化时的重命名逻辑
}

#[tokio::test]
async fn test_cleanup_old_backups() {
    // 测试过期清理
}
```

## 错误处理

| 场景 | 处理方式 |
|------|----------|
| 单表导出失败 | 记录错误，继续其他表 |
| S3 上传失败 | 重试 3 次，失败记录错误 |
| 本地磁盘满 | 记录错误，尝试清理旧备份 |
| 数据库连接断开 | 等待重连，跳过本次备份 |
| 哈希计算超时 | 设置超时时间，默认 5 分钟 |

## 日志输出

```
2026-06-30T06:00:00Z INFO  backup::runner Starting daily backup
2026-06-30T06:00:01Z INFO  backup::runner Table auth_user: hash=abc123, exporting...
2026-06-30T06:00:05Z INFO  backup::runner Table auth_user: saved to local + S3
2026-06-30T06:00:06Z INFO  backup::runner Table photo_photo: hash=def456, unchanged
2026-06-30T06:00:06Z INFO  backup::runner Table photo_photo: renamed backup
2026-06-30T06:00:10Z INFO  backup::runner Cleanup: removed 3 expired backups
2026-06-30T06:00:10Z INFO  backup::runner Daily backup completed in 10s
```

## 监控指标（可选，未来扩展）

```rust
backup_total{table="auth_user", status="success"} 1
backup_duration_seconds{table="auth_user"} 12.5
backup_file_size_bytes{table="auth_user"} 1048576
backup_last_success_timestamp 1719705600
```

## 实现步骤

1. 创建 `domains/backup/` 模块结构
2. 实现 `BackupConfig` 和 `BackupState`
3. 实现 `TableHasher` 哈希计算
4. 实现 `CsvExporter` CSV 导出
5. 实现 `BackupStorage` 存储管理
6. 实现 `BackupScheduler` 定时调度
7. 实现 `BackupRunner` 主流程
8. 集成到 `AppState` 和 `main.rs`
9. 添加配置文件支持
10. 编写单元测试和集成测试
