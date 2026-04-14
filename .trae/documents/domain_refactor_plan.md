# 领域驱动重构计划：将功能归还到 Domain

## 目标

将控制器从 server 层移动到各自的 domain 领域中，实现真正的领域驱动设计，精简依赖关系，并细化 feature 配置。

## 当前问题分析

### 1. 控制器位置问题
- `user_controller.rs` 在 server 中，应该属于 user domain
- `auth_controller.rs` 在 server 中，应该属于 auth domain
- photo 相关控制器在 server 中，应该属于 photo domain

### 2. 中间件位置问题
- `auth.rs` 认证中间件保留在 server（用户要求）
- `validator.rs` 通用功能保留在 server
- 中间件中的 `#[cfg]` 全部移除（用户要求）

### 3. AppState 耦合问题
- AppState 包含了所有领域的状态
- 每个领域都依赖完整的 AppState

### 4. Feature 配置问题
- server 的 feature 配置不够精细
- domain 内部没有细分 feature（如 controller 应该作为单独 feature）

## 重构方案

### 阶段一：Domain Feature 细化设计

#### 1.1 User Domain Features

```toml
[features]
default = []
controller = ["axum", "dep:validator"]  # 控制器功能
metrics = ["common/metrics", "dep:metrics"]  # 监控指标
avatar = ["dep:oss"]  # 头像上传功能
```

**Feature 依赖关系：**
- `controller`: 启用 axum 和 validator，提供 HTTP 接口
- `metrics`: 启用性能监控
- `avatar`: 启用头像上传功能（依赖 OSS）

#### 1.2 Auth Domain Features

```toml
[features]
default = []
controller = ["axum", "dep:validator"]  # 控制器功能
metrics = ["common/metrics", "dep:metrics"]  # 监控指标
```

**Feature 依赖关系：**
- `controller`: 启用 axum 和 validator，提供 HTTP 接口
- `metrics`: 启用性能监控

#### 1.3 Photo Domain Features

```toml
[features]
default = []
controller = ["axum", "dep:validator", "dep:tower-http"]  # 控制器功能
metrics = ["common/metrics", "dep:metrics"]  # 监控指标
```

**Feature 依赖关系：**
- `controller`: 启用 axum、validator 和 tower-http，提供 HTTP 接口
- `metrics`: 启用性能监控

### 阶段二：创建 Domain 控制器模块

#### 2.1 User Domain

**文件结构：**
```
domains/user/
├── src/
│   ├── controller/
│   │   ├── mod.rs
│   │   └── user_controller.rs
│   ├── models/
│   │   └── mod.rs
│   ├── services/
│   │   └── mod.rs
│   └── lib.rs
└── Cargo.toml
```

**Cargo.toml 更新：**
```toml
[package]
name = "user"
version = "0.1.0"
edition = "2024"

[features]
default = []
controller = ["dep:axum", "dep:validator"]
metrics = ["common/metrics", "dep:metrics"]
avatar = ["dep:oss"]

[dependencies]
# 基础依赖
common = { path = "../../common" }
entities = { path = "../../entities" }
img_url_generator = { path = "../../libs/img_url_generator" }

# 数据库和缓存
sea-orm = { workspace = true }
deadpool-redis = { workspace = true }

# 序列化
serde = { workspace = true }

# 时间和追踪
chrono = { workspace = true }
tracing = { workspace = true }

# 异步运行时
tokio = { workspace = true }

# 可选依赖 - 控制器功能
axum = { workspace = true, optional = true }
validator = { workspace = true, optional = true }

# 可选依赖 - 头像上传
oss = { path = "../../libs/oss", optional = true }

# 可选依赖 - 监控
metrics = { workspace = true, optional = true }
```

**lib.rs 更新：**
```rust
pub mod models;
pub mod services;

#[cfg(feature = "controller")]
pub mod controller;

#[cfg(feature = "controller")]
pub use controller::UserController;

pub use services::*;
pub use models::*;
```

#### 2.2 Auth Domain

**文件结构：**
```
domains/auth/
├── src/
│   ├── controller/
│   │   ├── mod.rs
│   │   └── auth_controller.rs
│   ├── models/
│   │   └── mod.rs
│   ├── service/
│   │   └── mod.rs
│   └── lib.rs
└── Cargo.toml
```

**Cargo.toml 更新：**
```toml
[package]
name = "auth"
version = "0.1.0"
edition = "2024"

[features]
default = []
controller = ["dep:axum", "dep:validator"]
metrics = ["common/metrics", "dep:metrics"]

[dependencies]
common = { path = "../../common" }
entities = { path = "../../entities" }
email = { path = "../../libs/email" }
img_url_generator = { path = "../../libs/img_url_generator" }

sea-orm = { workspace = true }
deadpool-redis = { workspace = true }
serde = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }
tokio = { workspace = true }

# 可选依赖 - 控制器功能
axum = { workspace = true, optional = true }
validator = { workspace = true, optional = true }

# 可选依赖 - 监控
metrics = { workspace = true, optional = true }
```

**lib.rs 更新：**
```rust
pub mod models;
pub mod service;

#[cfg(feature = "controller")]
pub mod controller;

#[cfg(feature = "controller")]
pub use controller::AuthController;

pub use service::*;
pub use models::*;
```

#### 2.3 Photo Domain

**文件结构：**
```
domains/photo/
├── src/
│   ├── controller/
│   │   ├── mod.rs
│   │   ├── photo_controller.rs
│   │   ├── collection_controller.rs
│   │   ├── comment_controller.rs
│   │   ├── face_controller.rs
│   │   └── timeline_controller.rs
│   ├── models/
│   │   └── ...
│   ├── services/
│   │   └── ...
│   ├── mappers/
│   │   └── ...
│   ├── clustering/
│   │   └── ...
│   ├── utils/
│   │   └── ...
│   └── lib.rs
└── Cargo.toml
```

**Cargo.toml 更新：**
```toml
[package]
name = "photo"
version = "0.1.0"
edition = "2024"

[features]
default = []
controller = ["dep:axum", "dep:validator", "dep:tower-http", "dep:tokio-util"]
metrics = ["common/metrics", "dep:metrics"]

[dependencies]
common = { path = "../../common" }
entities = { path = "../../entities" }
oss = { path = "../../libs/oss" }
img_url_generator = { path = "../../libs/img_url_generator" }
face_engine = { path = "../../libs/face_engine" }

# 数据库和缓存
sea-orm = { workspace = true }
deadpool-redis = { workspace = true }

# 序列化
serde = { workspace = true }
serde_json = { workspace = true }

# 时间和追踪
chrono = { workspace = true }
tracing = { workspace = true }

# 异步运行时
tokio = { workspace = true }
tokio-util = { workspace = true, optional = true }

# 图像处理
image = { workspace = true }
md5 = { workspace = true }
uuid = { workspace = true }

# 并行计算
num_cpus = { workspace = true }
rayon = "1.11.0"
ndarray = { version = "0.16", features = ["rayon"] }
futures = { workspace = true }

# 拼音和编码
pinyin = { workspace = true }
base64 = { workspace = true }

# 可选依赖 - 控制器功能
axum = { workspace = true, optional = true }
validator = { workspace = true, optional = true }
tower-http = { version = "0.6.8", features = ["cors", "trace"], optional = true }

# 可选依赖 - 监控
metrics = { workspace = true, optional = true }
```

**lib.rs 更新：**
```rust
pub mod models;
pub mod services;
pub mod clustering;
pub mod mappers;
pub mod utils;

#[cfg(feature = "controller")]
pub mod controller;

#[cfg(feature = "controller")]
pub use controller::{
    PhotoController,
    CollectionController,
    CommentController,
    FaceController,
    TimelineController,
};

pub use services::photo_service::FaceTask;
pub use models::*;
pub use services::*;
```

### 阶段三：中间件简化

#### 3.1 auth.rs 简化

**修改前：**
```rust
#[cfg(any(feature = "user", feature = "photo"))]
use crate::state::AppState;
#[cfg(any(feature = "user", feature = "photo"))]
use axum::extract::Request;
// ... 更多 #[cfg]

#[cfg(any(feature = "user", feature = "photo"))]
pub struct UserId(pub i64);

#[cfg(any(feature = "user", feature = "photo"))]
pub async fn auth_middleware(...) { ... }
```

**修改后：**
```rust
use crate::state::AppState;
use axum::extract::Request;
use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;
use common::constants::RedisKeys;
use common::error::AppError;
use common::utils::RedisExt;
use std::sync::Arc;

#[derive(Copy, Clone)]
pub struct UserId(pub i64);

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next
) -> Result<Response, AppError> {
    // ... 实现代码不变
}
```

**保留位置：** `server/src/middlewares/auth.rs`

### 阶段四：Server 层精简

#### 4.1 更新后的 server 结构

```
server/
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── state.rs
│   ├── metrics.rs
│   ├── middlewares/
│   │   ├── mod.rs
│   │   ├── auth.rs      # 认证中间件（保留，移除 #[cfg]）
│   │   ├── trace_id.rs  # 链路追踪
│   │   └── validator.rs # 请求验证
│   └── utils/
│       ├── mod.rs
│       └── client_ip.rs
└── Cargo.toml
```

#### 4.2 更新 server/Cargo.toml

```toml
[package]
name = "server"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "memory-seek-backend-rs"
path = "src/main.rs"

[features]
default = ["auth", "user", "photo", "metrics"]
auth = ["dep:auth", "auth/controller"]
user = ["dep:user", "auth", "user/controller"]
photo = ["dep:photo", "user", "photo/controller", "dep:face_engine"]
basic_metrics = [
    "dep:metrics",
    "dep:metrics-exporter-prometheus",
    "dep:metrics-tracing-context",
    "dep:metrics-util",
    "dep:metrics-process"
]
metrics = ["basic_metrics", "common/metrics"]

[dependencies]
# Domain crates
common = { path = "../common" }
user = { path = "../domains/user", optional = true }
auth = { path = "../domains/auth", optional = true }
photo = { path = "../domains/photo", optional = true }
entities = { path = "../entities" }
face_engine = { path = "../libs/face_engine", optional = true }

# Web framework
axum = { workspace = true }
tower = "0.5.3"
tower-http = { version = "0.6.8", features = ["cors", "trace"] }

# Serialization
serde = { workspace = true }
validator = { workspace = true }

# Database & Cache
sea-orm = { workspace = true }
deadpool-redis = { workspace = true }

# Async runtime
tokio = { workspace = true }
tokio-util = { workspace = true }

# Logging & Tracing
tracing = { version = "0.1.44", features = ["attributes", "std"] }
tracing-subscriber = { version = "0.3.23", features = ["registry", "env-filter", "fmt"] }
log = "0.4.29"

# Configuration
config = { workspace = true }
dotenvy = { workspace = true }

# Utilities
chrono = { workspace = true }
uuid = { version = "1.23.0", features = ["v4"] }

# Metrics (optional)
metrics = { workspace = true, optional = true }
metrics-exporter-prometheus = { version = "0.18.1", optional = true }
metrics-tracing-context = { version = "0.18.1", optional = true }
metrics-util = { version = "0.20.1", optional = true }
metrics-process = { version = "2.4.3", optional = true }

# Other
axum-client-ip = "1.3.1"
tikv-jemallocator = { version = "0.6.1", features = ["profiling"] }
```

**关键变化：**
- `auth` feature 启用 `auth/controller`
- `user` feature 启用 `user/controller`
- `photo` feature 启用 `photo/controller`

### 阶段五：控制器代码迁移

#### 5.1 user_controller.rs 迁移

**移动到：** `domains/user/src/controller/user_controller.rs`

**关键修改：**
1. 移除 `use crate::middlewares::auth::UserId;`
2. 添加 `use common::middleware::UserId;`（或直接在 user domain 定义）
3. 移除 `use crate::state::AppState;`
4. 使用 user domain 自己的 state 或直接使用参数

**简化后的控制器：**
```rust
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use axum::extract::Multipart;
use common::error::AppError;
use common::r::R;
use common::utils::ResultExt;
use common::middleware::UserId;
use entities::user::UserDTO;
use std::sync::Arc;
use validator::Validate;

use crate::models::{ChangeNicknameRequest, ChangePasswordRequest, GetUserInfoBatchRequest, InviterCodeDTO, UserInfoVO};
use crate::services as user_service;
use crate::UserState;

pub struct UserController;

impl UserController {
    pub fn routes() -> Router<Arc<UserState>> {
        let router = Router::new()
            .route("/info", get(Self::get_user_info))
            .route("/inviter-code", get(Self::generate_inviter_code))
            .route("/nickname", post(Self::change_nickname))
            .route("/password", post(Self::change_password))
            .route("/logout", get(Self::logout))
            .route("/info/batch", post(Self::get_user_info_batch));
        
        #[cfg(feature = "avatar")]
        let router = router.route("/avatar", post(Self::upload_avatar));
        
        router
    }

    // ... 方法实现
}
```

#### 5.2 auth_controller.rs 迁移

**移动到：** `domains/auth/src/controller/auth_controller.rs`

**简化后的控制器：**
```rust
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::Router;
use common::error::AppError;
use common::r::R;
use common::utils::ResultExt;
use entities::user::UserDTO;
use std::sync::Arc;
use validator::Validate;

use crate::models::{AccessTokenResponse, LoginRequest, RegisterRequest, SendEmailCodeRequest};
use crate::service as auth_service;
use crate::AuthState;

pub struct AuthController;

impl AuthController {
    pub fn routes() -> Router<Arc<AuthState>> {
        Router::new()
            .route("/login", post(Self::login))
            .route("/register", post(Self::register))
            .route("/email-verify-code", get(Self::send_email_code))
            .route("/access-token", get(Self::refresh_access_token))
    }

    // ... 方法实现
}
```

### 阶段六：State 定义

#### 6.1 User State

**文件：** `domains/user/src/state.rs`

```rust
use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;

#[cfg(feature = "avatar")]
use oss::S3Client;

pub struct UserState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub encryption_key: [u8; 32],
    
    #[cfg(feature = "avatar")]
    pub s3_client: S3Client,
}
```

#### 6.2 Auth State

**文件：** `domains/auth/src/state.rs`

```rust
use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;
use email::EmailClient;

pub struct AuthState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub email_client: EmailClient,
    pub encryption_key: [u8; 32],
}
```

#### 6.3 Photo State

**文件：** `domains/photo/src/state.rs`

```rust
use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;
use oss::S3Client;
use tokio::sync::mpsc;
use crate::FaceTask;

pub struct PhotoState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub s3_client: S3Client,
    pub face_tx: Option<mpsc::Sender<FaceTask>>,
    pub encryption_key: [u8; 32],
}
```

#### 6.4 Server AppState

**文件：** `server/src/state.rs`

```rust
use deadpool_redis::Pool;
use sea_orm::DatabaseConnection;

use email::EmailClient;
use oss::S3Client;
use tokio::sync::mpsc;
use photo::FaceTask;

pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Pool,
    pub encryption_key: [u8; 32],
    pub email_client: EmailClient,
    pub s3_client: S3Client,
    pub face_tx: Option<mpsc::Sender<FaceTask>>,
}

// 提供转换方法
impl AppState {
    pub fn user_state(&self) -> user::UserState {
        user::UserState {
            db: self.db.clone(),
            redis: self.redis.clone(),
            encryption_key: self.encryption_key,
            #[cfg(feature = "avatar")]
            s3_client: self.s3_client.clone(),
        }
    }
    
    pub fn auth_state(&self) -> auth::AuthState {
        auth::AuthState {
            db: self.db.clone(),
            redis: self.redis.clone(),
            email_client: self.email_client.clone(),
            encryption_key: self.encryption_key,
        }
    }
    
    pub fn photo_state(&self) -> photo::PhotoState {
        photo::PhotoState {
            db: self.db.clone(),
            redis: self.redis.clone(),
            s3_client: self.s3_client.clone(),
            face_tx: self.face_tx.clone(),
            encryption_key: self.encryption_key,
        }
    }
}
```

### 阶段七：main.rs 更新

#### 7.1 导入更新

```rust
mod config;
mod middlewares;
mod state;
mod utils;

#[cfg(feature = "basic_metrics")]
mod metrics;

// Domain controllers
use auth::AuthController;
use user::UserController;
use photo::{PhotoController, CollectionController, CommentController, FaceController, TimelineController};

// Middleware
use middlewares::auth::auth_middleware;

#[cfg(feature = "basic_metrics")]
use crate::metrics::{init_metrics_system, spawn_monitoring_tasks};

#[cfg(feature = "basic_metrics")]
use metrics_tracing_context::MetricsLayer;
```

#### 7.2 路由配置

```rust
// Public routes
let mut public_routers = Router::new();

public_routers = public_routers.nest("/auth", AuthController::routes());
public_routers = public_routers.nest("/photo/image", PhotoController::public_routes());

// Protected routes
let mut protected_routes = Router::new();

protected_routes = protected_routes.nest("/user", UserController::routes());
protected_routes = protected_routes
    .nest("/photo/photo", PhotoController::routes())
    .nest("/photo/collection", CollectionController::routes())
    .nest("/photo/face", FaceController::routes())
    .nest("/photo/comment", CommentController::routes())
    .nest("/photo/timeline", TimelineController::routes());

let protected_routes = protected_routes.route_layer(axum::middleware::from_fn_with_state(
    state.clone(),
    auth_middleware,
));
```

## 实施步骤

### 步骤 1：更新 domain 的 Cargo.toml（30 分钟）
1. 更新 `domains/user/Cargo.toml` - 添加 features 和可选依赖
2. 更新 `domains/auth/Cargo.toml` - 添加 features 和可选依赖
3. 更新 `domains/photo/Cargo.toml` - 添加 features 和可选依赖

### 步骤 2：创建 State 定义（30 分钟）
1. 创建 `domains/user/src/state.rs`
2. 创建 `domains/auth/src/state.rs`
3. 创建 `domains/photo/src/state.rs`
4. 更新 `server/src/state.rs` - 添加转换方法

### 步骤 3：创建控制器目录并移动文件（30 分钟）
1. 创建 `domains/user/src/controller/` 并移动 `user_controller.rs`
2. 创建 `domains/auth/src/controller/` 并移动 `auth_controller.rs`
3. 创建 `domains/photo/src/controller/` 并移动 photo 相关控制器

### 步骤 4：更新控制器代码（1 小时）
1. 修改导入路径
2. 使用各自的 State 类型
3. 移除对 server 层的依赖

### 步骤 5：更新 domain 的 lib.rs（15 分钟）
1. 添加 `pub mod controller;`（条件编译）
2. 导出公共接口

### 步骤 6：简化中间件（15 分钟）
1. 移除 `server/src/middlewares/auth.rs` 中的所有 `#[cfg]`
2. 确保中间件正常工作

### 步骤 7：更新 server 层（30 分钟）
1. 删除 `server/src/controllers/` 目录
2. 更新 `server/Cargo.toml`
3. 更新 `server/src/main.rs`

### 步骤 8：测试验证（30 分钟）
1. 运行 `cargo check` 验证编译
2. 运行所有 feature 组合测试
3. 运行压力测试验证功能

## 依赖关系图

```
server
├── auth (feature: auth)
│   ├── auth/controller (启用控制器)
│   ├── common
│   ├── entities
│   ├── email
│   └── img_url_generator
├── user (feature: user, depends on: auth)
│   ├── user/controller (启用控制器)
│   ├── user/avatar (可选，头像上传)
│   ├── common
│   ├── entities
│   └── img_url_generator
└── photo (feature: photo, depends on: user)
    ├── photo/controller (启用控制器)
    ├── common
    ├── entities
    ├── oss
    ├── face_engine
    └── img_url_generator
```

## Feature 配置表

| Domain | Feature | 依赖 | 说明 |
|--------|---------|------|------|
| user | controller | axum, validator | HTTP 接口 |
| user | metrics | common/metrics, metrics | 性能监控 |
| user | avatar | oss | 头像上传 |
| auth | controller | axum, validator | HTTP 接口 |
| auth | metrics | common/metrics, metrics | 性能监控 |
| photo | controller | axum, validator, tower-http, tokio-util | HTTP 接口 |
| photo | metrics | common/metrics, metrics | 性能监控 |

## 优势

1. **清晰的领域边界**：每个 domain 包含自己的控制器、服务、模型、状态
2. **精细的 feature 控制**：controller 作为独立 feature，按需启用
3. **精简的依赖**：每个 domain 只依赖自己需要的库
4. **简化的中间件**：移除不必要的条件编译，代码更清晰
5. **更好的可测试性**：domain 可以独立测试
6. **符合 DDD 原则**：真正的领域驱动设计

## 风险与注意事项

1. **State 克隆开销**：AppState 的克隆可能带来性能开销（但 Arc 克隆开销很小）
2. **中间件共享**：UserId 需要在 common 或 user domain 中定义
3. **Feature 组合**：确保所有 feature 组合都能正常工作
4. **循环依赖**：确保 domain 之间没有循环依赖

## 预计工作量

- 步骤 1-2：1 小时
- 步骤 3-5：2 小时
- 步骤 6-7：1 小时
- 步骤 8：30 分钟
- **总计：约 4.5 小时**
