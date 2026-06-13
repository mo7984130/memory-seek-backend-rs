# 测试架构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 memory-seek-backend-rs 服务搭建完整的测试架构，包括 Rust 集成测试和 k6 负载测试

**Architecture:** 使用 Docker Compose 管理测试环境（PostgreSQL、Redis、MinIO），Rust 集成测试验证功能正确性，k6 负载测试获取性能数据

**Tech Stack:** Rust、Axum、SeaORM、Docker Compose、k6、MinIO

---

## 文件结构

```
memory-seek-backend-rs-new/
├── tests/
│   ├── integration/
│   │   ├── mod.rs
│   │   ├── helpers/
│   │   │   ├── mod.rs
│   │   │   ├── app.rs
│   │   │   ├── auth.rs
│   │   │   ├── db.rs
│   │   │   └── minio.rs
│   │   ├── auth/
│   │   │   ├── mod.rs
│   │   │   ├── login.rs
│   │   │   ├── register.rs
│   │   │   └── token.rs
│   │   ├── photo/
│   │   │   ├── mod.rs
│   │   │   ├── upload.rs
│   │   │   ├── query.rs
│   │   │   └── delete.rs
│   │   └── user/
│   │       ├── mod.rs
│   │       └── profile.rs
│   └── load/
│       ├── docker-compose.yml
│       ├── Makefile
│       ├── config.test.json
│       ├── fixtures/
│       │   └── test.jpg
│       ├── results/
│       └── scripts/
│           ├── auth.js
│           └── photo.js
├── .github/
│   └── workflows/
│       └── test.yml
└── Cargo.toml
```

---

## Task 1: 创建目录结构和 Docker Compose

**Files:**
- Create: `tests/load/docker-compose.yml`
- Create: `tests/load/Makefile`
- Create: `tests/load/config.test.json`
- Create: `tests/load/fixtures/` (目录)
- Create: `tests/load/results/` (目录)

- [ ] **Step 1: 创建 Docker Compose 文件**

```yaml
# tests/load/docker-compose.yml
version: '3.8'

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: memory_seek_test
      POSTGRES_USER: test
      POSTGRES_PASSWORD: test
    ports:
      - "5433:5432"
    volumes:
      - ../../docs/sql/init.sql:/docker-entrypoint-initdb.d/init.sql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U test"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    ports:
      - "6380:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 5s
      retries: 5

  minio:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: minioadmin
      MINIO_ROOT_PASSWORD: minioadmin
    ports:
      - "9000:9000"
      - "9001:9001"
    volumes:
      - minio_data:/data
    healthcheck:
      test: ["CMD", "mc", "ready", "local"]
      interval: 5s
      timeout: 5s
      retries: 5

  app:
    build:
      context: ../..
      dockerfile: server/Dockerfile
    ports:
      - "3000:3000"
    environment:
      MEMORY_SEEK_CONFIG_PATH: /app/config.test.json
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
      minio:
        condition: service_healthy
    profiles:
      - load

volumes:
  minio_data:
```

- [ ] **Step 2: 创建测试配置文件**

```json
// tests/load/config.test.json
{
  "database_config": {
    "url": "postgres://test:test@localhost:5433/memory_seek_test",
    "max_connections": 10
  },
  "redis_config": {
    "url": "redis://localhost:6380"
  },
  "oss_config": {
    "bucket": "test-bucket",
    "region": "us-east-1",
    "endpoint": "http://localhost:9000",
    "access_key": "minioadmin",
    "secret_key": "minioadmin"
  },
  "token_cipher_config": {
    "key": "test-secret-key-for-testing-only"
  }
}
```

- [ ] **Step 3: 创建 Makefile**

```makefile
# tests/load/Makefile

.PHONY: test-unit test-integration test-load test-all clean

test-unit:
	cargo test --lib

test-integration:
	docker compose up -d postgres redis minio
	sleep 10
	cargo test -p server --features "auth,user,photo" -- --test-threads=1
	docker compose down -v

test-load: test-load-auth test-load-photo

test-load-auth:
	docker compose --profile load up -d
	sleep 30
	k6 run scripts/auth.js --out json=results/auth.json
	docker compose --profile load down -v

test-load-photo:
	docker compose --profile load up -d
	sleep 30
	k6 run scripts/photo.js --out json=results/photo.json
	docker compose --profile load down -v

test-all: test-unit test-integration test-load

clean:
	docker compose --profile load down -v
	rm -rf results/*.json results/*.csv
```

- [ ] **Step 4: 创建 fixtures 和 results 目录**

```bash
mkdir -p tests/load/fixtures tests/load/results
```

- [ ] **Step 5: 验证 Docker Compose 配置**

```bash
cd tests/load
docker compose config
```

Expected: 输出解析后的配置，无错误

- [ ] **Step 6: 提交**

```bash
git add tests/load/
git commit -m "test: 添加 Docker Compose 测试环境配置"
```

---

## Task 2: 更新 Cargo.toml 添加测试依赖

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 添加 dev-dependencies**

在 `Cargo.toml` 的 `[workspace.dependencies]` 之后添加：

```toml
[workspace.dev-dependencies]
reqwest = { version = "0.12", features = ["json", "multipart"] }
tokio-test = "0.4"
serde_json = "1.0"
```

- [ ] **Step 2: 在 server/Cargo.toml 添加测试依赖**

```toml
# server/Cargo.toml
[dev-dependencies]
common = { path = "../common" }
entities = { path = "../entities" }
auth = { path = "../domains/auth" }
user = { path = "../domains/user" }
photo = { path = "../domains/photo" }
reqwest = { workspace = true }
tokio = { workspace = true }
tokio-test = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
axum = { workspace = true }
```

- [ ] **Step 3: 验证依赖可以编译**

```bash
cargo check -p server
```

Expected: 编译成功（可能有未实现的测试文件警告）

- [ ] **Step 4: 提交**

```bash
git add Cargo.toml server/Cargo.toml
git commit -m "test: 添加集成测试依赖"
```

---

## Task 3: 创建测试辅助模块 - 数据库清理

**Files:**
- Create: `tests/integration/mod.rs`
- Create: `tests/integration/helpers/mod.rs`
- Create: `tests/integration/helpers/db.rs`

- [ ] **Step 1: 创建 integration 测试入口**

```rust
// tests/integration/mod.rs
mod helpers;
mod auth;
mod photo;
mod user;
```

- [ ] **Step 2: 创建 helpers 模块入口**

```rust
// tests/integration/helpers/mod.rs
pub mod app;
pub mod auth;
pub mod db;
pub mod minio;

pub use app::build_test_state;
pub use auth::get_test_token;
```

- [ ] **Step 3: 创建数据库清理 guard**

```rust
// tests/integration/helpers/db.rs
use sea_orm::{DatabaseConnection, EntityTrait, ConnectionTrait};

/// 测试数据清理 guard
/// 测试结束时自动清理数据库中的测试数据
pub struct CleanupGuard {
    db: DatabaseConnection,
}

impl CleanupGuard {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        // 在测试结束时清理数据
        // 注意：Drop 不能是 async，需要使用 block_on
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 清理顺序：先删除有外键依赖的表
                let tables = vec![
                    "comment_likes",
                    "comments",
                    "collection_photos",
                    "collections",
                    "photos",
                    "refresh_tokens",
                    "users",
                ];
                for table in tables {
                    let sql = format!("DELETE FROM {} WHERE true", table);
                    db.execute(sea_orm::Statement::from_string(
                        sea_orm::DatabaseBackend::Postgres,
                        sql,
                    ))
                    .await
                    .ok();
                }
            });
        });
    }
}
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p server
```

- [ ] **Step 5: 提交**

```bash
git add tests/integration/
git commit -m "test: 添加测试辅助模块和数据库清理 guard"
```

---

## Task 4: 创建测试辅助模块 - 应用状态构建

**Files:**
- Create: `tests/integration/helpers/app.rs`

- [ ] **Step 1: 编写 build_test_state 函数**

```rust
// tests/integration/helpers/app.rs
use std::sync::Arc;
use server::config::AppConfig;
use server::state::AppState;
use server::setup::bases::AppBase;
use server::setup::libs::AppLibs;
use super::db::CleanupGuard;

/// 构建测试用 AppState
/// 从 config.test.json 加载配置，初始化数据库和 Redis 连接
pub async fn build_test_state() -> (AppState, CleanupGuard) {
    let config_path = std::env::var("TEST_CONFIG_PATH")
        .unwrap_or_else(|_| "tests/load/config.test.json".to_string());
    
    let cfg = AppConfig::from_json(&config_path);
    let bases = AppBase::init(&cfg).await;
    let libs = AppLibs::init(&cfg);
    let state = AppState::from(bases, libs);
    
    let guard = CleanupGuard::new(state.db.clone());
    (state, guard)
}

/// 构建带自定义 MinIO 端点的测试 AppState
pub async fn build_test_state_with_minio(minio_endpoint: &str) -> (AppState, CleanupGuard) {
    // 类似 build_test_state，但覆盖 OSS 配置
    let config_path = std::env::var("TEST_CONFIG_PATH")
        .unwrap_or_else(|_| "tests/load/config.test.json".to_string());
    
    let cfg = AppConfig::from_json(&config_path);
    // TODO: 覆盖 cfg.oss_config.endpoint 为 minio_endpoint
    let bases = AppBase::init(&cfg).await;
    let libs = AppLibs::init(&cfg);
    let state = AppState::from(bases, libs);
    
    let guard = CleanupGuard::new(state.db.clone());
    (state, guard)
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p server
```

- [ ] **Step 3: 提交**

```bash
git add tests/integration/helpers/app.rs
git commit -m "test: 添加测试状态构建函数"
```

---

## Task 5: 创建测试辅助模块 - 认证辅助

**Files:**
- Create: `tests/integration/helpers/auth.rs`

- [ ] **Step 1: 编写认证辅助函数**

```rust
// tests/integration/helpers/auth.rs
use axum::Router;
use axum::http::StatusCode;
use serde_json::json;

/// 测试用户凭证
pub struct TestUser {
    pub username: String,
    pub email: String,
    pub password: String,
    pub nickname: String,
}

impl TestUser {
    pub fn new(id: u32) -> Self {
        Self {
            username: format!("testuser_{}", id),
            email: format!("test_{}@example.com", id),
            password: "Test123456".to_string(),
            nickname: format!("Test User {}", id),
        }
    }
}

/// 注册测试用户
pub async fn register_test_user(
    client: &reqwest::Client,
    base_url: &str,
    user: &TestUser,
) -> Result<(), reqwest::Error> {
    client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": user.username,
            "email": user.email,
            "password": user.password,
            "nickname": user.nickname,
        }))
        .send()
        .await?;
    Ok(())
}

/// 登录并获取 access_token
pub async fn login_test_user(
    client: &reqwest::Client,
    base_url: &str,
    user: &TestUser,
) -> Result<String, reqwest::Error> {
    let resp = client
        .post(&format!("{}/login", base_url))
        .json(&json!({
            "account": user.email,
            "password": user.password,
        }))
        .send()
        .await?;
    
    let body: serde_json::Value = resp.json().await?;
    Ok(body["data"]["access_token"].as_str().unwrap().to_string())
}

/// 注册并登录，返回 token
pub async fn get_test_token(
    client: &reqwest::Client,
    base_url: &str,
    user_id: u32,
) -> String {
    let user = TestUser::new(user_id);
    register_test_user(client, base_url, &user).await.unwrap();
    login_test_user(client, base_url, &user).await.unwrap()
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p server
```

- [ ] **Step 3: 提交**

```bash
git add tests/integration/helpers/auth.rs
git commit -m "test: 添加认证测试辅助函数"
```

---

## Task 6: 创建测试辅助模块 - MinIO 辅助

**Files:**
- Create: `tests/integration/helpers/minio.rs`

- [ ] **Step 1: 编写 MinIO 辅助函数**

```rust
// tests/integration/helpers/minio.rs
use reqwest::Client;

/// 确保测试 bucket 存在
pub async fn ensure_test_bucket(
    minio_endpoint: &str,
    bucket_name: &str,
) -> Result<(), reqwest::Error> {
    let client = Client::new();
    
    // 使用 MinIO API 创建 bucket
    let url = format!("{}/{}", minio_endpoint, bucket_name);
    client
        .put(&url)
        .basic_auth("minioadmin", Some("minioadmin"))
        .send()
        .await?;
    
    Ok(())
}

/// 清空测试 bucket
pub async fn clear_test_bucket(
    minio_endpoint: &str,
    bucket_name: &str,
) -> Result<(), reqwest::Error> {
    let client = Client::new();
    
    // 列出 bucket 中的对象
    let url = format!("{}/{}?list-type=2", minio_endpoint, bucket_name);
    let resp = client
        .get(&url)
        .basic_auth("minioadmin", Some("minioadmin"))
        .send()
        .await?;
    
    // TODO: 解析 XML 响应并删除对象
    
    Ok(())
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p server
```

- [ ] **Step 3: 提交**

```bash
git add tests/integration/helpers/minio.rs
git commit -m "test: 添加 MinIO 测试辅助函数"
```

---

## Task 7: 创建 Auth 集成测试 - 登录

**Files:**
- Create: `tests/integration/auth/mod.rs`
- Create: `tests/integration/auth/login.rs`

- [ ] **Step 1: 创建 auth 模块入口**

```rust
// tests/integration/auth/mod.rs
mod login;
mod register;
mod token;
```

- [ ] **Step 2: 编写登录成功测试**

```rust
// tests/integration/auth/login.rs
use reqwest::Client;
use serde_json::json;
use crate::helpers::{build_test_state, auth::TestUser};

#[tokio::test]
async fn test_login_success() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    // 先注册用户
    let user = TestUser::new(1);
    client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": user.username,
            "email": user.email,
            "password": user.password,
            "nickname": user.nickname,
        }))
        .send()
        .await
        .unwrap();
    
    // 登录
    let resp = client
        .post(&format!("{}/login", base_url))
        .json(&json!({
            "account": user.email,
            "password": user.password,
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["data"]["access_token"].as_str().is_some());
    assert!(body["data"]["refresh_token"].as_str().is_some());
}
```

- [ ] **Step 3: 编写登录失败测试（密码错误）**

```rust
#[tokio::test]
async fn test_login_wrong_password() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    // 先注册用户
    let user = TestUser::new(2);
    client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": user.username,
            "email": user.email,
            "password": user.password,
            "nickname": user.nickname,
        }))
        .send()
        .await
        .unwrap();
    
    // 使用错误密码登录
    let resp = client
        .post(&format!("{}/login", base_url))
        .json(&json!({
            "account": user.email,
            "password": "wrong_password",
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 400);
}
```

- [ ] **Step 4: 编写登录失败测试（用户不存在）**

```rust
#[tokio::test]
async fn test_login_user_not_found() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let resp = client
        .post(&format!("{}/login", base_url))
        .json(&json!({
            "account": "nonexistent@example.com",
            "password": "Test123456",
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 400);
}
```

- [ ] **Step 5: 提交**

```bash
git add tests/integration/auth/
git commit -m "test: 添加登录集成测试"
```

---

## Task 8: 创建 Auth 集成测试 - 注册

**Files:**
- Create: `tests/integration/auth/register.rs`

- [ ] **Step 1: 编写注册成功测试**

```rust
// tests/integration/auth/register.rs
use reqwest::Client;
use serde_json::json;
use crate::helpers::build_test_state;

#[tokio::test]
async fn test_register_success() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let resp = client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": "newuser",
            "email": "newuser@example.com",
            "password": "Test123456",
            "nickname": "New User",
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["data"]["id"].as_i64().is_some());
}
```

- [ ] **Step 2: 编写注册失败测试（邮箱已存在）**

```rust
#[tokio::test]
async fn test_register_duplicate_email() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    // 第一次注册
    client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": "user1",
            "email": "duplicate@example.com",
            "password": "Test123456",
            "nickname": "User 1",
        }))
        .send()
        .await
        .unwrap();
    
    // 第二次注册相同邮箱
    let resp = client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": "user2",
            "email": "duplicate@example.com",
            "password": "Test123456",
            "nickname": "User 2",
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 400);
}
```

- [ ] **Step 3: 编写注册失败测试（用户名已存在）**

```rust
#[tokio::test]
async fn test_register_duplicate_username() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    // 第一次注册
    client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": "duplicateuser",
            "email": "user1@example.com",
            "password": "Test123456",
            "nickname": "User 1",
        }))
        .send()
        .await
        .unwrap();
    
    // 第二次注册相同用户名
    let resp = client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": "duplicateuser",
            "email": "user2@example.com",
            "password": "Test123456",
            "nickname": "User 2",
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 400);
}
```

- [ ] **Step 4: 提交**

```bash
git add tests/integration/auth/register.rs
git commit -m "test: 添加注册集成测试"
```

---

## Task 9: 创建 Auth 集成测试 - Token 刷新

**Files:**
- Create: `tests/integration/auth/token.rs`

- [ ] **Step 1: 编写 Token 刷新成功测试**

```rust
// tests/integration/auth/token.rs
use reqwest::Client;
use serde_json::json;
use crate::helpers::{build_test_state, auth::TestUser};

#[tokio::test]
async fn test_refresh_token_success() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    // 注册并登录
    let user = TestUser::new(10);
    client
        .post(&format!("{}/register", base_url))
        .json(&json!({
            "username": user.username,
            "email": user.email,
            "password": user.password,
            "nickname": user.nickname,
        }))
        .send()
        .await
        .unwrap();
    
    let login_resp = client
        .post(&format!("{}/login", base_url))
        .json(&json!({
            "account": user.email,
            "password": user.password,
        }))
        .send()
        .await
        .unwrap();
    
    let login_body: serde_json::Value = login_resp.json().await.unwrap();
    let user_id = login_body["data"]["id"].as_i64().unwrap();
    let refresh_token = login_body["data"]["refresh_token"].as_str().unwrap();
    
    // 刷新 token
    let resp = client
        .post(&format!("{}/token", base_url))
        .header("x-user-id", user_id.to_string())
        .header("x-refresh-token", refresh_token)
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["data"]["access_token"].as_str().is_some());
}
```

- [ ] **Step 2: 编写 Token 刷新失败测试（无效 token）**

```rust
#[tokio::test]
async fn test_refresh_token_invalid() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let resp = client
        .post(&format!("{}/token", base_url))
        .header("x-user-id", "1")
        .header("x-refresh-token", "invalid-token")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 401);
}
```

- [ ] **Step 3: 编写 Token 刷新失败测试（缺少 header）**

```rust
#[tokio::test]
async fn test_refresh_token_missing_header() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    // 缺少 x-user-id
    let resp = client
        .post(&format!("{}/token", base_url))
        .header("x-refresh-token", "some-token")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 400);
    
    // 缺少 x-refresh-token
    let resp = client
        .post(&format!("{}/token", base_url))
        .header("x-user-id", "1")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 400);
}
```

- [ ] **Step 4: 提交**

```bash
git add tests/integration/auth/token.rs
git commit -m "test: 添加 Token 刷新集成测试"
```

---

## Task 10: 创建 Photo 集成测试 - 上传

**Files:**
- Create: `tests/integration/photo/mod.rs`
- Create: `tests/integration/photo/upload.rs`

- [ ] **Step 1: 创建 photo 模块入口**

```rust
// tests/integration/photo/mod.rs
mod upload;
mod query;
mod delete;
```

- [ ] **Step 2: 编写图片上传成功测试**

```rust
// tests/integration/photo/upload.rs
use reqwest::Client;
use crate::helpers::{build_test_state, auth::get_test_token};

#[tokio::test]
async fn test_upload_photo_success() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    // 获取 token
    let token = get_test_token(&client, base_url, 100).await;
    
    // 创建测试图片（1x1 像素的 JPEG）
    let test_image = create_test_jpeg();
    
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(test_image)
                .file_name("test.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );
    
    let resp = client
        .post(&format!("{}/photo", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["data"]["id"].as_i64().is_some());
    assert!(body["data"]["file_id"].as_str().is_some());
}

fn create_test_jpeg() -> Vec<u8> {
    // 最小的合法 JPEG 文件
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
        0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
        0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
        0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
        0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
        0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0A, 0x0B, 0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03,
        0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
        0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06,
        0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
        0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72,
        0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
        0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
        0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
        0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75,
        0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3,
        0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
        0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9,
        0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
        0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
        0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01,
        0x00, 0x00, 0x3F, 0x00, 0x7B, 0x94, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xFF, 0xD9,
    ]
}
```

- [ ] **Step 3: 编写图片上传失败测试（未认证）**

```rust
#[tokio::test]
async fn test_upload_photo_unauthorized() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let test_image = create_test_jpeg();
    
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(test_image)
                .file_name("test.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );
    
    // 不带 Authorization header
    let resp = client
        .post(&format!("{}/photo", base_url))
        .multipart(form)
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 401);
}
```

- [ ] **Step 4: 提交**

```bash
git add tests/integration/photo/
git commit -m "test: 添加图片上传集成测试"
```

---

## Task 11: 创建 Photo 集成测试 - 查询

**Files:**
- Create: `tests/integration/photo/query.rs`

- [ ] **Step 1: 编写图片分页查询测试**

```rust
// tests/integration/photo/query.rs
use reqwest::Client;
use crate::helpers::{build_test_state, auth::get_test_token};

#[tokio::test]
async fn test_get_photos_cursor() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let token = get_test_token(&client, base_url, 200).await;
    
    // 先上传几张图片
    for i in 0..3 {
        let test_image = create_test_jpeg();
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(test_image)
                    .file_name(format!("test_{}.jpg", i))
                    .mime_str("image/jpeg")
                    .unwrap(),
            );
        
        client
            .post(&format!("{}/photo", base_url))
            .header("Authorization", format!("Bearer {}", token))
            .multipart(form)
            .send()
            .await
            .unwrap();
    }
    
    // 查询图片列表
    let resp = client
        .get(&format!("{}/photo?limit=2", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    
    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["data"]["items"].as_array().unwrap();
    assert!(items.len() <= 2);
    assert!(body["data"]["next_cursor"].as_str().is_some());
}

fn create_test_jpeg() -> Vec<u8> {
    // 最小的合法 JPEG 文件（1x1 像素）
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
        0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
        0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
        0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
        0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
        0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0A, 0x0B, 0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03,
        0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
        0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06,
        0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
        0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72,
        0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
        0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
        0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
        0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75,
        0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3,
        0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
        0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9,
        0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
        0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
        0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01,
        0x00, 0x00, 0x3F, 0x00, 0x7B, 0x94, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xFF, 0xD9,
    ]
}
```

- [ ] **Step 2: 编写 MD5 查重测试**

```rust
#[tokio::test]
async fn test_md5s_exist() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let token = get_test_token(&client, base_url, 201).await;
    
    // 上传一张图片
    let test_image = create_test_jpeg();
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(test_image.clone())
                .file_name("test.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );
    
    client
        .post(&format!("{}/photo", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    
    // 查询 MD5 是否存在
    let md5 = format!("{:x}", md5::compute(&test_image));
    
    let resp = client
        .post(&format!("{}/photo/check-existence", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "md5s": [md5, "nonexistent_md5"]
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    
    let body: serde_json::Value = resp.json().await.unwrap();
    let results = body["data"].as_array().unwrap();
    assert_eq!(results[0], true);  // 已存在
    assert_eq!(results[1], false); // 不存在
}
```

- [ ] **Step 3: 提交**

```bash
git add tests/integration/photo/query.rs
git commit -m "test: 添加图片查询集成测试"
```

---

## Task 12: 创建 Photo 集成测试 - 删除

**Files:**
- Create: `tests/integration/photo/delete.rs`

- [ ] **Step 1: 编写图片删除成功测试**

```rust
// tests/integration/photo/delete.rs
use reqwest::Client;
use serde_json::json;
use crate::helpers::{build_test_state, auth::get_test_token};

#[tokio::test]
async fn test_delete_photos_success() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let token = get_test_token(&client, base_url, 300).await;
    
    // 上传图片
    let test_image = create_test_jpeg();
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(test_image)
                .file_name("test.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );
    
    let upload_resp = client
        .post(&format!("{}/photo", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    
    let upload_body: serde_json::Value = upload_resp.json().await.unwrap();
    let photo_id = upload_body["data"]["id"].as_i64().unwrap();
    
    // 删除图片
    let resp = client
        .delete(&format!("{}/photo", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "photo_ids": [photo_id]
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
}

fn create_test_jpeg() -> Vec<u8> {
    // 最小的合法 JPEG 文件（1x1 像素）
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
        0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
        0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
        0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
        0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
        0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0A, 0x0B, 0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03,
        0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
        0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06,
        0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
        0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72,
        0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
        0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
        0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
        0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75,
        0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3,
        0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
        0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9,
        0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
        0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
        0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01,
        0x00, 0x00, 0x3F, 0x00, 0x7B, 0x94, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xFF, 0xD9,
    ]
}
```

- [ ] **Step 2: 编写图片删除失败测试（无权限）**

```rust
#[tokio::test]
async fn test_delete_photos_unauthorized() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    // 不带 token
    let resp = client
        .delete(&format!("{}/photo", base_url))
        .json(&json!({
            "photo_ids": [1]
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 401);
}
```

- [ ] **Step 3: 提交**

```bash
git add tests/integration/photo/delete.rs
git commit -m "test: 添加图片删除集成测试"
```

---

## Task 13: 创建 User 集成测试

**Files:**
- Create: `tests/integration/user/mod.rs`
- Create: `tests/integration/user/profile.rs`

- [ ] **Step 1: 创建 user 模块入口**

```rust
// tests/integration/user/mod.rs
mod profile;
```

- [ ] **Step 2: 编写用户信息查询测试**

```rust
// tests/integration/user/profile.rs
use reqwest::Client;
use crate::helpers::{build_test_state, auth::get_test_token};

#[tokio::test]
async fn test_get_user_profile() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let token = get_test_token(&client, base_url, 400).await;
    
    let resp = client
        .get(&format!("{}/user/profile", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["data"]["id"].as_i64().is_some());
    assert!(body["data"]["username"].as_str().is_some());
    assert!(body["data"]["email"].as_str().is_some());
}
```

- [ ] **Step 3: 编写用户信息查询失败测试（未认证）**

```rust
#[tokio::test]
async fn test_get_user_profile_unauthorized() {
    let (state, _guard) = build_test_state().await;
    let client = Client::new();
    let base_url = "http://localhost:3000";
    
    let resp = client
        .get(&format!("{}/user/profile", base_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 401);
}
```

- [ ] **Step 4: 提交**

```bash
git add tests/integration/user/
git commit -m "test: 添加用户信息集成测试"
```

---

## Task 14: 创建 k6 认证负载测试

**Files:**
- Create: `tests/load/scripts/auth.js`

- [ ] **Step 1: 编写 k6 认证测试脚本**

```javascript
// tests/load/scripts/auth.js
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// 自定义指标
const loginErrorRate = new Rate('login_errors');
const loginDuration = new Trend('login_duration');

export const options = {
  stages: [
    { duration: '30s', target: 50 },   // 逐步加压到 50 用户
    { duration: '1m', target: 50 },    // 保持 50 用户 1 分钟
    { duration: '30s', target: 100 },  // 加压到 100 用户
    { duration: '1m', target: 100 },   // 保持 100 用户
    { duration: '30s', target: 0 },    // 逐步降压
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],  // 95% 请求 < 500ms
    login_errors: ['rate<0.1'],        // 错误率 < 10%
  },
};

const BASE_URL = __ENV.APP_URL || 'http://localhost:3000';

// 用户池隔离：每个 VU*1000+ITER 生成唯一用户
function getUserCredentials(vuId, iterId) {
  const userId = vuId * 1000 + iterId;
  return {
    account: `loadtest_user_${userId}@test.com`,
    password: 'Test123456',
  };
}

// 预注册用户（setup 阶段只执行一次）
export function setup() {
  const totalUsers = 100 * 1000; // max_vus * max_iterations
  const registered = [];

  for (let i = 0; i < totalUsers; i++) {
    const { account, password } = getUserCredentials(0, i);
    const res = http.post(`${BASE_URL}/register`, JSON.stringify({
      username: `loadtest_${i}`,
      email: account,
      password: password,
      nickname: `Test User ${i}`,
    }), {
      headers: { 'Content-Type': 'application/json' },
    });

    if (res.status === 200) {
      registered.push(account);
    }
  }

  return { registeredCount: registered.length };
}

export default function (data) {
  // 每次迭代使用不同用户，确保 token 唯一
  const { account, password } = getUserCredentials(__VU, __ITER);

  // 登录
  const loginRes = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password,
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  check(loginRes, {
    'login status is 200': (r) => r.status === 200,
    'login has token': (r) => r.json('data.access_token') !== undefined,
  });

  loginErrorRate.add(loginRes.status !== 200);
  loginDuration.add(loginRes.timings.duration);

  if (loginRes.status !== 200) {
    console.error(`Login failed for ${account}: ${loginRes.body}`);
    return;
  }

  const token = loginRes.json('data.access_token');

  // 后续请求使用该 token
  // 由于每次迭代都是新用户，不会出现 token 冲突
  sleep(1);
}
```

- [ ] **Step 2: 验证 k6 脚本语法**

```bash
cd tests/load
k6 run --dry-run scripts/auth.js
```

Expected: 无语法错误

- [ ] **Step 3: 提交**

```bash
git add tests/load/scripts/auth.js
git commit -m "test: 添加 k6 认证负载测试脚本"
```

---

## Task 15: 创建 k6 图片负载测试

**Files:**
- Create: `tests/load/scripts/photo.js`
- Create: `tests/load/fixtures/test.jpg` (测试图片)

- [ ] **Step 1: 创建测试图片**

```bash
# 使用 ImageMagick 创建 1x1 像素的测试图片
convert -size 1x1 xc:white tests/load/fixtures/test.jpg
```

或者手动创建一个最小的 JPEG 文件。

- [ ] **Step 2: 编写 k6 图片测试脚本**

```javascript
// tests/load/scripts/photo.js
import http from 'k6/http';
import { check } from 'k6';
import { SharedArray } from 'k6/data';

// 共享测试图片
const testImages = new SharedArray('images', function () {
  return [
    open('./fixtures/test.jpg', 'b'),
  ];
});

export const options = {
  scenarios: {
    upload: {
      executor: 'shared-iterations',
      vus: 20,
      iterations: 200,  // 每个 VU 10 次迭代
      maxDuration: '5m',
    },
  },
  thresholds: {
    http_req_duration: ['p(95)<1000'],  // 上传较慢，放宽到 1s
  },
};

const BASE_URL = __ENV.APP_URL || 'http://localhost:3000';

// 每个 VU 独立的 token
const tokens = {};

function getToken() {
  if (tokens[__VU]) {
    return tokens[__VU];
  }

  // 每个 VU 使用独立用户登录
  const account = `loadtest_photo_${__VU}@test.com`;
  const loginRes = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password: 'Test123456',
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  if (loginRes.status === 200) {
    tokens[__VU] = loginRes.json('data.access_token');
  }

  return tokens[__VU];
}

export default function () {
  const token = getToken();
  if (!token) return;

  const headers = { Authorization: `Bearer ${token}` };

  // 上传
  const image = testImages[__VU % testImages.length];
  const uploadRes = http.post(`${BASE_URL}/photo`, {
    file: http.file(image, 'test.jpg', 'image/jpeg'),
  }, { headers });

  check(uploadRes, {
    'upload success': (r) => r.status === 200,
  });

  // 查询
  const queryRes = http.get(`${BASE_URL}/photo?limit=20`, { headers });
  check(queryRes, {
    'query success': (r) => r.status === 200,
  });
}
```

- [ ] **Step 3: 验证 k6 脚本语法**

```bash
cd tests/load
k6 run --dry-run scripts/photo.js
```

Expected: 无语法错误

- [ ] **Step 4: 提交**

```bash
git add tests/load/scripts/photo.js tests/load/fixtures/
git commit -m "test: 添加 k6 图片负载测试脚本"
```

---

## Task 16: 创建 CI/CD 配置

**Files:**
- Create: `.github/workflows/test.yml`

- [ ] **Step 1: 创建 GitHub Actions 工作流**

```yaml
# .github/workflows/test.yml
name: Test & Benchmark

on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]

jobs:
  integration-test:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache dependencies
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Start test services
        run: |
          docker compose -f tests/load/docker-compose.yml up -d postgres redis minio
          sleep 10
          docker compose -f tests/load/docker-compose.yml exec -T postgres pg_isready -U test
          docker compose -f tests/load/docker-compose.yml exec -T redis redis-cli ping

      - name: Run integration tests
        run: cargo test -p server --features "auth,user,photo" -- --test-threads=1
        env:
          DATABASE_URL: postgres://test:test@localhost:5433/memory_seek_test
          REDIS_URL: redis://localhost:6380
          MINIO_ENDPOINT: http://localhost:9000

      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: test-results
          path: target/test-results/

  load-test:
    runs-on: ubuntu-latest
    needs: integration-test
    if: github.ref == 'refs/heads/main' || github.ref == 'refs/heads/master'

    steps:
      - uses: actions/checkout@v4

      - name: Install k6
        run: |
          sudo gpg -k
          sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D68
          echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
          sudo apt-get update
          sudo apt-get install k6

      - name: Start full environment
        run: |
          docker compose -f tests/load/docker-compose.yml --profile load up -d
          sleep 30

      - name: Run load tests
        run: |
          cd tests/load
          k6 run scripts/auth.js --out json=results/auth.json
          k6 run scripts/photo.js --out json=results/photo.json

      - name: Upload load test results
        uses: actions/upload-artifact@v4
        with:
          name: load-test-results
          path: tests/load/results/

      - name: Cleanup
        if: always()
        run: docker compose -f tests/load/docker-compose.yml --profile load down -v
```

- [ ] **Step 2: 验证 YAML 语法**

```bash
# 使用 yamllint 或在线验证器检查语法
yamllint .github/workflows/test.yml
```

Expected: 无语法错误

- [ ] **Step 3: 提交**

```bash
git add .github/workflows/
git commit -m "ci: 添加测试和负载测试 CI/CD 配置"
```

---

## Task 17: 端到端验证

**Files:**
- 无新文件，验证整个测试架构

- [ ] **Step 1: 启动测试环境**

```bash
cd tests/load
docker compose up -d postgres redis minio
sleep 10
```

Expected: 所有容器启动成功

- [ ] **Step 2: 运行集成测试**

```bash
cargo test -p server --features "auth,user,photo" -- --test-threads=1
```

Expected: 所有测试通过

- [ ] **Step 3: 运行 k6 负载测试（快速验证）**

```bash
cd tests/load
docker compose --profile load up -d
sleep 30
k6 run --duration=30s --vus=10 scripts/auth.js
k6 run --duration=30s --vus=5 scripts/photo.js
docker compose --profile load down -v
```

Expected: 测试完成，输出性能指标

- [ ] **Step 4: 清理环境**

```bash
cd tests/load
docker compose down -v
rm -rf results/*.json results/*.csv
```

- [ ] **Step 5: 最终提交**

```bash
git add .
git commit -m "test: 完成测试架构搭建

- Rust 集成测试（auth、photo、user）
- k6 负载测试（认证、图片）
- Docker Compose 测试环境
- CI/CD 配置"
```

---

## 总结

本计划共 17 个任务，按顺序执行可以搭建完整的测试架构：

1. **Task 1-2**: 基础设施（Docker、依赖）
2. **Task 3-6**: 测试辅助模块
3. **Task 7-13**: Rust 集成测试
4. **Task 14-15**: k6 负载测试
5. **Task 16**: CI/CD 配置
6. **Task 17**: 端到端验证

每个任务都是独立的，可以单独验证和提交。
