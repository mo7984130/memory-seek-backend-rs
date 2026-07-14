use backup::runner::BackupRunner;
use backup::{BackupConfig, BackupState};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
use std::sync::Arc;

use crate::helpers::test_config;

/// 获取测试数据库连接
async fn get_test_db() -> DatabaseConnection {
    let cfg = test_config();
    Database::connect(&cfg.database.url)
        .await
        .expect("连接测试数据库失败")
}

/// 插入测试用户，返回 (user_id, username)
///
/// 确保 auth_user 表不为空，以便 CSV 导出测试可以正常工作。
async fn insert_test_user(db: &DatabaseConnection, suffix: &str) -> (i64, String) {
    let username = format!("backup_test_{}", suffix);
    let email = format!("backup_test_{}@test.com", suffix);

    // 先清理可能存在的旧数据
    let _ = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!(
                "DELETE FROM auth_user WHERE username = '{}'",
                username.replace('\'', "''")
            ),
        ))
        .await;

    // 插入测试用户
    db.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        format!(
            "INSERT INTO auth_user (username, email, password, nickname, inviter) \
             VALUES ('{}', '{}', 'hashed_password', 'Backup Test', 0)",
            username.replace('\'', "''"),
            email.replace('\'', "''")
        ),
    ))
    .await
    .expect("插入测试用户失败");

    // 获取插入的 ID（SeaORM execute 不直接返回 ID，用查询获取）
    let row = db
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!(
                "SELECT id FROM auth_user WHERE username = '{}'",
                username.replace('\'', "''")
            ),
        ))
        .await
        .expect("查询测试用户 ID 失败")
        .expect("测试用户不存在");

    let id: i64 = row.try_get("", "id").expect("获取用户 ID 失败");
    (id, username)
}

/// 清理测试用户
async fn cleanup_test_user(db: &DatabaseConnection, user_id: i64) {
    // 清理关联的 photo 数据（如果有）
    let _ = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("DELETE FROM photo_photo_like WHERE user_id = {}", user_id),
        ))
        .await;
    let _ = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("DELETE FROM photo_comment_like WHERE user_id = {}", user_id),
        ))
        .await;
    let _ = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("DELETE FROM photo_comment WHERE user_id = {}", user_id),
        ))
        .await;
    let _ = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!(
                "DELETE FROM photo_collection_photo WHERE collection_id IN \
                 (SELECT id FROM photo_collection WHERE user_id = {})",
                user_id
            ),
        ))
        .await;
    let _ = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("DELETE FROM photo_collection WHERE user_id = {}", user_id),
        ))
        .await;
    let _ = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("DELETE FROM photo_photo WHERE user_id = {}", user_id),
        ))
        .await;

    // 删除测试用户
    let _ = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("DELETE FROM auth_user WHERE id = {}", user_id),
        ))
        .await;
}

// ==================== BackupConfig 验证测试 ====================

/// 测试有效配置验证通过
#[tokio::test]
async fn test_backup_config_validate_success() {
    let config = BackupConfig {
        enabled: true,
        schedule: "0 0 6 * * *".to_string(),
        local_path: "/tmp/test-backup".to_string(),
        s3_prefix: "test-backup/".to_string(),
        retention_days: 3,
        tables: Some(vec!["auth_user".to_string()]),
    };

    assert!(config.validate().is_ok(), "有效配置应该通过验证");
}

/// 测试空 local_path 验证失败
#[tokio::test]
async fn test_backup_config_validate_empty_local_path() {
    let config = BackupConfig {
        enabled: true,
        schedule: "0 0 6 * * *".to_string(),
        local_path: "".to_string(),
        s3_prefix: "backup/".to_string(),
        retention_days: 3,
        tables: None,
    };

    let result = config.validate();
    assert!(result.is_err(), "空 local_path 应该验证失败");
    assert!(result.unwrap_err().contains("local_path"));
}

/// 测试 retention_days 为 0 验证失败
#[tokio::test]
async fn test_backup_config_validate_zero_retention_days() {
    let config = BackupConfig {
        enabled: true,
        schedule: "0 0 6 * * *".to_string(),
        local_path: "/tmp/test-backup".to_string(),
        s3_prefix: "backup/".to_string(),
        retention_days: 0,
        tables: None,
    };

    let result = config.validate();
    assert!(result.is_err(), "retention_days=0 应该验证失败");
    assert!(result.unwrap_err().contains("retention_days"));
}

/// 测试配置默认值（通过 serde 反序列化）
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

    assert!(config.enabled);
    assert_eq!(config.schedule, "0 0 6 * * *");
    assert_eq!(config.local_path, "/tmp/test");
    assert_eq!(config.s3_prefix, "backup/");
    assert_eq!(config.retention_days, 3);
    assert!(config.tables.is_none());
}

/// 测试从 JSON 反序列化配置
#[tokio::test]
async fn test_backup_config_from_json() {
    let json = r#"{
        "enabled": true,
        "schedule": "0 30 8 * * *",
        "local_path": "/var/backups/test",
        "s3_prefix": "daily-backup/",
        "retention_days": 7,
        "tables": ["auth_user", "photo_photo"]
    }"#;

    let config: BackupConfig = serde_json::from_str(json).expect("反序列化失败");

    assert!(config.enabled);
    assert_eq!(config.schedule, "0 30 8 * * *");
    assert_eq!(config.local_path, "/var/backups/test");
    assert_eq!(config.s3_prefix, "daily-backup/");
    assert_eq!(config.retention_days, 7);
    assert_eq!(
        config.tables,
        Some(vec!["auth_user".to_string(), "photo_photo".to_string()])
    );
}

/// 测试最小 JSON（只提供必填字段，其余用默认值）
#[tokio::test]
async fn test_backup_config_minimal_json() {
    let json = r#"{}"#;

    let config: BackupConfig = serde_json::from_str(json).expect("反序列化失败");

    // 使用 serde(default) 的字段应该有默认值
    assert!(config.enabled); // default_enabled = true
    assert_eq!(config.schedule, "0 0 6 * * *"); // default_schedule
    assert_eq!(config.local_path, "/var/backups/memory-seek"); // default_local_path
    assert_eq!(config.s3_prefix, "backup/"); // default_s3_prefix
    assert_eq!(config.retention_days, 3); // default_retention_days
    assert!(config.tables.is_none()); // default = None
}

// ==================== 集成测试（需要数据库） ====================

/// 测试从测试配置加载备份配置
#[tokio::test]
async fn test_backup_config_from_test_config() {
    let cfg = test_config();

    // 检查配置中是否有 backup 字段
    // 注意：AppConfig 中 backup 是 Option<BackupConfig>
    // 当前测试配置文件中包含 backup 配置
    let backup_cfg = cfg.backup;
    assert!(backup_cfg.is_some(), "测试配置应包含 backup 配置");

    let config = backup_cfg.unwrap();
    assert!(config.enabled);
    assert_eq!(config.schedule, "0 0 6 * * *");
    assert_eq!(config.retention_days, 3);
    assert!(config.tables.is_none());
}

/// 测试数据库连接和表哈希计算
#[tokio::test]
async fn test_table_hasher_with_real_db() {
    let db = get_test_db().await;

    // 计算 auth_user 表的哈希
    let hash = backup::hasher::TableHasher::compute(&db, "auth_user")
        .await
        .expect("计算表哈希失败");

    // 哈希应该是 64 字符的十六进制字符串（SHA256）
    assert_eq!(hash.len(), 64, "哈希长度应为 64");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "哈希应为十六进制字符串"
    );
}

/// 测试获取所有表名
#[tokio::test]
async fn test_get_all_tables_with_real_db() {
    let db = get_test_db().await;

    let tables = backup::hasher::TableHasher::get_all_tables(&db)
        .await
        .expect("获取表名失败");

    // 至少应该有 auth_user 表
    assert!(!tables.is_empty(), "数据库应至少有一个表");
    assert!(
        tables.contains(&"auth_user".to_string()),
        "应包含 auth_user 表"
    );
}

/// 测试 CSV 导出
#[tokio::test]
async fn test_csv_export_with_real_db() {
    let db = get_test_db().await;
    let temp_dir = std::env::temp_dir().join("backup_test_csv");

    // 插入测试数据确保表不为空
    let (user_id, _) = insert_test_user(&db, "csv_exp").await;

    // 确保临时目录存在
    std::fs::create_dir_all(&temp_dir).expect("创建临时目录失败");

    // 导出 auth_user 表
    let (csv_path, hash) = backup::exporter::CsvExporter::export(&db, "auth_user", &temp_dir)
        .await
        .expect("CSV 导出失败");

    // 验证 CSV 文件存在
    assert!(csv_path.exists(), "CSV 文件应存在");
    assert!(
        csv_path.to_string_lossy().ends_with(".csv"),
        "文件应为 CSV 格式"
    );

    // 验证哈希
    assert_eq!(hash.len(), 64, "哈希长度应为 64");

    // 清理
    let _ = std::fs::remove_file(&csv_path);
    let _ = std::fs::remove_dir(&temp_dir);
    cleanup_test_user(&db, user_id).await;
}

/// 测试完整备份流程（需要数据库和 S3）
///
/// 此测试执行完整的备份流程：
/// 1. 创建 BackupState
/// 2. 执行 BackupRunner::execute()
/// 3. 验证备份结果
#[tokio::test]
async fn test_full_backup_flow() {
    let cfg = test_config();
    let db = get_test_db().await;

    // 插入测试数据确保表不为空
    let (user_id, _) = insert_test_user(&db, "full_flow").await;

    // 获取 S3 客户端
    let s3_config = cfg.s3.expect("测试配置应包含 S3 配置");
    let s3_client = Arc::new(oss::S3Client::new(&s3_config));

    // 创建备份配置（只备份 auth_user 表）
    let backup_config = BackupConfig {
        enabled: true,
        schedule: "0 0 6 * * *".to_string(),
        local_path: "/tmp/test-backup-flow".to_string(),
        s3_prefix: "test-backup/".to_string(),
        retention_days: 1,
        tables: Some(vec!["auth_user".to_string()]),
    };

    // 验证配置
    assert!(backup_config.validate().is_ok(), "备份配置应有效");

    // 创建 BackupState
    let state = Arc::new(BackupState::new(db.clone(), s3_client, backup_config));

    // 确保目录存在
    state.ensure_dirs().expect("创建备份目录失败");

    // 执行备份
    let result = BackupRunner::execute(state).await.expect("备份执行失败");

    // 验证结果：至少导出一个表
    assert!(
        result.exported > 0 || result.skipped > 0 || result.renamed > 0,
        "至少应导出、跳过或重命名一个表"
    );
    assert_eq!(result.failed, 0, "不应有失败的表（当前测试环境）");

    // 清理测试数据和临时目录
    cleanup_test_user(&db, user_id).await;
    let _ = std::fs::remove_dir_all("/tmp/test-backup-flow");
}

/// 测试备份状态创建和目录初始化
#[tokio::test]
async fn test_backup_state_creation() {
    let cfg = test_config();
    let db = get_test_db().await;

    let s3_config = cfg.s3.expect("测试配置应包含 S3 配置");
    let s3_client = Arc::new(oss::S3Client::new(&s3_config));

    let backup_config = BackupConfig {
        enabled: true,
        schedule: "0 0 6 * * *".to_string(),
        local_path: "/tmp/test-state-creation".to_string(),
        s3_prefix: "test-backup/".to_string(),
        retention_days: 3,
        tables: None,
    };

    let state = BackupState::new(db, s3_client, backup_config);

    // 确保目录创建成功
    state.ensure_dirs().expect("创建目录失败");
    assert!(state.temp_dir.exists(), "临时目录应存在");
    assert!(
        std::path::Path::new(&state.config.local_path).exists(),
        "本地备份目录应存在"
    );

    // 清理
    let _ = std::fs::remove_dir_all("/tmp/test-state-creation");
}
