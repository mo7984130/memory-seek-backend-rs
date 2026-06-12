# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Rust 后端服务，基于 Axum + SeaORM + PostgreSQL + Redis + S3 (MinIO)。采用模块化架构，通过 feature flags 控制业务模块加载。

## 常用命令

### 构建

```bash
# 完整构建（所有模块）
cargo build --features "auth,user,photo"

# 仅构建特定模块
cargo build --features auth
cargo build --features "auth,photo"
```

### 测试

```bash
# 启动测试服务（PostgreSQL + Redis + MinIO）
podman compose -f tests/load/docker-compose.yml up -d postgres redis minio

# 运行集成测试（必须串行执行）
cargo test --test integration auth --features auth -- --test-threads=1
cargo test --test integration photo --features "auth,photo" -- --test-threads=1
cargo test --test integration user --features "auth,user" -- --test-threads=1

# 运行单个测试
cargo test --test integration auth::register::test_register_success --features auth -- --test-threads=1

# 运行所有集成测试
cargo test --test integration --features "auth,user,photo" -- --test-threads=1

# 运行单元测试
cargo test --lib
```

### 负载测试

```bash
# 完整负载测试环境
podman compose -f tests/load/docker-compose.yml --profile load up -d
k6 run tests/load/scripts/auth.js
k6 run tests/load/scripts/photo.js
```

## 代码架构

### Workspace 结构

```
├── common/          — 跨业务域共享基础设施（错误处理、提取器、工具函数）
├── entities/        — Sea-ORM 数据库实体定义
├── libs/
│   ├── email/       — 邮件发送客户端
│   ├── img_url_generator/ — 图片 URL 生成器
│   └── oss/         — S3 对象存储客户端
├── domains/
│   ├── constants/   — 业务常量定义
│   ├── auth/        — 认证模块（注册、登录、token 刷新）
│   ├── user/        — 用户模块（个人信息、头像、密码）
│   └── photo/       — 照片模块（上传、收藏、评论、时间线）
└── server/          — HTTP 服务器入口和配置
```

### 模块化架构

每个业务模块（auth/user/photo）遵循相同模式：

```
domains/<module>/
├── Cargo.toml      — feature flags: controller, metrics
├── src/
│   ├── lib.rs      — 导出 State 和 Controller
│   ├── state.rs    — 模块状态（db, redis, 等）
│   ├── controller/ — 路由定义和处理器
│   ├── services/   — 业务逻辑
│   └── models/     — 请求/响应模型
```

### ControllerRouter Trait

所有控制器实现 `ControllerRouter` trait：

```rust
pub trait ControllerRouter {
    type State: Send + Sync + 'static;

    fn protected_routes() -> Router<Arc<Self::State>>;  // 需要认证
    fn public_routes() -> Router<Arc<Self::State>>;     // 无需认证
}
```

### AppModule Trait

每个模块实现 `AppModule` trait 用于注册：

```rust
pub trait AppModule: Send + Sync + 'static {
    type State: Send + Sync + 'static;
    type Controller: ControllerRouter<State = Self::State> + 'static;

    fn build(state: &AppState, cfg: &AppConfig) -> Arc<Self::State>;
}
```

### 路由注册流程

1. `server/src/main.rs` 调用 `init_module(&state, &cfg)`
2. `ModuleRegistry` 按顺序注册各模块（auth → user → photo）
3. 返回 `(public_router, protected_router)` 元组
4. 合并路由并添加中间件（CORS、trace_id）

### AppState 结构

```rust
pub struct AppState {
    pub db: DatabaseConnection,      // PostgreSQL
    pub redis: Pool,                 // Redis 连接池
    pub token_cipher: Arc<TokenCipher>, // token 加密/解密
    pub s3_client: Arc<oss::S3Client>,  // S3 客户端（feature = "s3"）
}
```

## 测试架构

### 测试基础设施

- `helpers/app.rs`: `build_test_router()` 构建测试用 Router，包含 test_auth_middleware
- `helpers/auth.rs`: `TestUser`, `register_user()`, `login_user()`, `register_and_login()` 等辅助函数
- `helpers/db.rs`: `CleanupGuard` 测试结束清理数据库
- `helpers/minio.rs`: MinIO 辅助函数（photo 模块用）

### 认证中间件

测试使用 `x-test-user-id` 请求头注入用户 ID：

```rust
// 在测试中添加请求头
.header("x-test-user-id", user_id.to_string())
```

### 测试配置

- 配置文件: `tests/load/config.test.json`
- 数据库: `postgres://test:test@localhost:5433/memory_seek_test`
- Redis: `redis://localhost:6380`
- MinIO: `http://localhost:9000`

### CleanupGuard 使用

```rust
let (app, guard) = build_test_router().await;
// ... 执行测试 ...
guard.cleanup().await; // 清理数据库
```

## 统一响应格式

所有 API 返回 `R<T>` 结构：

```json
{
    "code": 200,       // HTTP 状态码
    "msg": null,       // 错误信息（成功时为 null）
    "data": { ... }    // 响应数据（错误时为 null）
}
```

使用 `ResultRExt::to_r_ok()` 转换 `Result<T, AppError>` 为 `Result<R<T>, AppError>`。

## 错误处理

使用 `AppError` 枚举：

```rust
pub enum AppError {
    Unauthorized,           // 401
    BadRequest(Cow<str>),   // 400
    NotFound(Cow<str>),     // 404
    Forbidden(Cow<str>),    // 403
    Conflict(Cow<str>),     // 409
    InternalServerError,    // 500
    Ignore,                 // 200（静默错误）
}
```

### 错误处理规范

| 场景 | 方法 | 说明 |
|------|------|------|
| `Option<T>` → `Result` | `ok_or_warn()` | None 时记录 warn 日志 |
| `Result<T, E>` → `AppError` | `trace_internal_err()` | Err 时记录 error 日志 |
| `Result<T, E>` → 自定义错误 | `trace_err()` | Err 时记录 error 日志 |
| `Result<T, E>` → 警告 | `trace_warn()` | Err 时记录 warn 日志 |
| 类型转换 | `map_err(Into::into)` | 无需日志 |

使用扩展 trait 处理错误：
- `OptionExt::ok_or_warn()`: None → AppError + warn 日志
- `OptionExt::ok_or_warn_bad_request()`: None → BadRequest + warn 日志
- `ResultErrExt::trace_err()`: Err → error 日志 + 自定义 AppError
- `ResultErrExt::trace_internal_err()`: Err → error 日志 + InternalServerError
- `ResultErrExt::trace_warn()`: Err → warn 日志 + AppError
- `ResultRExt::to_r_ok()`: Ok(T) → Ok(R::ok(T))

## Feature Flags

| Feature | 说明 |
|---------|------|
| `auth` | 认证模块（注册、登录、token） |
| `user` | 用户模块（个人信息、头像） |
| `photo` | 照片模块（上传、收藏、评论） |
| `metrics` | 性能监控 |
| `s3` | S3 对象存储 |

## 数据库

- ORM: Sea-ORM
- 数据库: PostgreSQL 16
- 向量扩展: pgvector（用于人脸特征）
- 初始化脚本: `docs/sql/init.sql`

## 依赖注入

模块状态通过 `Arc<T>` 共享：

```rust
// 在 handler 中提取状态
async fn handler(
    State(state): State<Arc<AuthState>>,
    ValidatedJson(req): ValidatedJson<LoginRequest>,
) -> Result<R<UserDTO>, AppError> {
    // ...
}
```

## 约定

- 所有请求体字段使用 camelCase（`#[serde(rename_all = "camelCase")]`）
- 所有 ID 返回为字符串（`"id": "123"` 而非 `"id": 123`）
- 测试必须串行运行（`--test-threads=1`），因为共享数据库
- 每个测试结束前调用 `guard.cleanup().await`
- 配置文件路径通过环境变量 `MEMORY_SEEK_CONFIG_PATH` 或 `TEST_CONFIG_PATH` 指定
