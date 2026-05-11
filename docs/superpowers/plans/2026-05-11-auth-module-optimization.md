# Auth 模块优化实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 auth 模块的安全性漏洞（验证码重放）和代码质量问题（接口冗余、类型语义、token 长度）。

**Architecture:** 修改集中在 `domains/auth` 模块的 service 层和 controller 层。service 函数签名统一改为接收 `&AuthState`，消除 controller 拆解字段的样板代码。同时修正内部辅助函数的返回类型和 token 生成长度。

**Tech Stack:** Rust, axum, deadpool-redis, sea-orm

---

### Task 1: 修正 `verify_email_verify_code` 返回类型

**Files:**
- Modify: `domains/auth/src/services/auth_service.rs:376-384` — `verify_email_verify_code` 函数
- Modify: `domains/auth/src/services/auth_service.rs:206-209` — `register` 中的调用处

- [ ] **Step 1: 修改 `verify_email_verify_code` 函数签名和实现**

将返回类型从 `Result<bool, AppError>` 改为 `Result<(), AppError>`，验证失败时直接返回错误：

```rust
/// 效验邮箱验证码（大小写不敏感）
async fn verify_email_verify_code(redis: &Pool, email: &str, code: &str) -> Result<(), AppError> {
    let stored_code: Option<String> = redis
        .get_as(&RedisKeys::user::email_verify_code(email))
        .await
        .trace_internal_err("redis_error", "验证邮箱验证码时 获取redis值错误")?;
    let code_upper = code.to_uppercase();
    match stored_code {
        Some(v) if v == code_upper => Ok(()),
        _ => Err(AppError::bad_request("邮箱验证码错误")),
    }
}
```

- [ ] **Step 2: 修改 `register` 中的调用处**

将:
```rust
verify_email_verify_code(redis, &req.email, &req.email_verify_code)
    .timed(metrics_timer_name!("register", "verify_email_code"))
    .await?
    .ok_or_warn("invalid_email_code", "邮箱验证码错误")?;
```

改为:
```rust
verify_email_verify_code(redis, &req.email, &req.email_verify_code)
    .timed(metrics_timer_name!("register", "verify_email_code"))
    .await?;
```

- [ ] **Step 3: 编译验证**

Run: `cargo build --features "auth"`
Expected: 编译成功，无 error

- [ ] **Step 4: Commit**

```bash
git add domains/auth/src/services/auth_service.rs
git commit -m "fix(auth): 修正 verify_email_verify_code 返回类型为 Result<(), AppError>"
```

---

### Task 2: 简化 controller-service 接口

**Files:**
- Modify: `domains/auth/src/services/auth_service.rs` — 所有 pub 函数签名
- Modify: `domains/auth/src/controller/auth_controller.rs` — 调用处

- [ ] **Step 1: 修改 `login` 函数签名**

将:
```rust
pub async fn login(
    db: &DatabaseConnection,
    redis: &Pool,
    req: LoginRequest,
    token_cipher: &TokenCipher,
) -> Result<UserDTO, AppError> {
```

改为:
```rust
pub async fn login(
    state: &AuthState,
    req: LoginRequest,
) -> Result<UserDTO, AppError> {
```

函数体内将 `db` 替换为 `&state.db`，`redis` 替换为 `&state.redis`，`token_cipher` 替换为 `&state.token_cipher`。

- [ ] **Step 2: 修改 `register` 函数签名**

将:
```rust
pub async fn register(
    db: &DatabaseConnection,
    redis: &Pool,
    req: RegisterRequest,
) -> Result<UserDTO, AppError> {
```

改为:
```rust
pub async fn register(
    state: &AuthState,
    req: RegisterRequest,
) -> Result<UserDTO, AppError> {
```

函数体内将 `db` 替换为 `&state.db`，`redis` 替换为 `&state.redis`。

- [ ] **Step 3: 修改 `send_email_code` 函数签名**

将:
```rust
pub async fn send_email_code(
    redis: &Pool,
    email_client: &EmailClient,
    req: SendEmailCodeRequest,
) -> Result<(), AppError> {
```

改为:
```rust
pub async fn send_email_code(
    state: &AuthState,
    req: SendEmailCodeRequest,
) -> Result<(), AppError> {
```

函数体内将 `redis` 替换为 `&state.redis`，`email_client` 替换为 `&state.email_client`。

- [ ] **Step 4: 修改 `refresh_access_token` 函数签名**

将:
```rust
pub async fn refresh_access_token(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    refresh_token: String,
) -> Result<AccessTokenResponse, AppError> {
```

改为:
```rust
pub async fn refresh_access_token(
    state: &AuthState,
    user_id: i64,
    refresh_token: String,
) -> Result<AccessTokenResponse, AppError> {
```

函数体内将 `db` 替换为 `&state.db`，`redis` 替换为 `&state.redis`。

- [ ] **Step 5: 更新 `services/mod.rs` 的 re-export**

确认 `services/mod.rs` 中的 `pub use` 无需修改（函数名未变）。

- [ ] **Step 6: 修改 controller 调用处**

将 `domains/auth/src/controller/auth_controller.rs` 中的四个调用改为：

```rust
async fn login(
    State(state): State<Arc<AuthState>>,
    ValidatedJson(req): ValidatedJson<LoginRequest>
) -> Result<R<UserDTO>, AppError> {
    auth_service::login(&state, req).await.into_ok_res()
}

async fn register(
    State(state): State<Arc<AuthState>>,
    ValidatedJson(payload): ValidatedJson<RegisterRequest>
) -> Result<R<UserDTO>, AppError> {
    auth_service::register(&state, payload).await.into_ok_res()
}

async fn send_email_code(
    State(state): State<Arc<AuthState>>,
    ValidatedJson(payload): ValidatedJson<SendEmailCodeRequest>
) -> Result<R<()>, AppError> {
    auth_service::send_email_code(&state, payload).await.into_ok_res()
}

async fn refresh_access_token(
    State(state): State<Arc<AuthState>>,
    headers: HeaderMap
) -> Result<R<AccessTokenResponse>, AppError> {
    let user_id = headers.get("x-user-id")
        .ok_or_else(|| AppError::bad_request("x-user-id 头缺失"))?
        .to_str()
        .map_err(|_| AppError::bad_request("x-user-id 格式非法"))?
        .parse::<i64>()
        .map_err(|_| AppError::bad_request("x-user-id 必须是数字"))?;

    tracing::Span::current().record("user_id", user_id);

    let refresh_token_str = headers.get("x-refresh-token")
        .ok_or_else(|| AppError::bad_request("x-refresh-token 头缺失"))?
        .to_str()
        .map_err(|_| AppError::bad_request("x-refresh-token 格式非法"))?
        .to_string();
    auth_service::refresh_access_token(&state, user_id, refresh_token_str).await.into_ok_res()
}
```

- [ ] **Step 7: 移除 controller 中不再需要的 import**

移除 `auth_controller.rs` 中不再直接使用的 import（如 `DatabaseConnection`, `Pool`, `TokenCipher` 等——如果它们仅因传参而存在的话）。实际检查：当前 controller 并未直接 import 这些类型，它们是通过 `auth_service::` 前缀调用的，所以无需修改 import。

- [ ] **Step 8: 编译验证**

Run: `cargo build --features "auth"`
Expected: 编译成功，无 error

- [ ] **Step 9: Commit**

```bash
git add domains/auth/src/services/auth_service.rs domains/auth/src/controller/auth_controller.rs
git commit -m "refactor(auth): 简化 controller-service 接口，统一传递 &AuthState"
```

---

### Task 3: 注册成功后清理邮箱验证码

**Files:**
- Modify: `domains/auth/src/services/auth_service.rs` — `register` 函数

- [ ] **Step 1: 在注册成功分支中添加 Redis DEL**

在 `register` 函数的 `Ok(user_model)` 分支中，`metrics_success!` 之前，添加删除验证码的逻辑：

```rust
Ok(user_model) => {
    // 删除已使用的邮箱验证码，防止重放
    let _ = redis
        .del(&redis_keys::user::email_verify_code(&req.email))
        .await
        .trace_internal_err("redis_error", "删除已使用邮箱验证码失败");

    metrics_success!("register");
    // ... 后续代码不变
}
```

注意：删除验证码失败不应阻断注册流程（用 `let _` 忽略错误），因为用户已经成功注册。

- [ ] **Step 2: 编译验证**

Run: `cargo build --features "auth"`
Expected: 编译成功，无 error

- [ ] **Step 3: Commit**

```bash
git add domains/auth/src/services/auth_service.rs
git commit -m "fix(auth): 注册成功后清理已使用的邮箱验证码，防止重放攻击"
```

---

### Task 4: 增加 access_token 长度

**Files:**
- Modify: `domains/auth/src/services/auth_service.rs` — `login` 和 `refresh_access_token` 函数

- [ ] **Step 1: 修改 `login` 中的 token 生成**

将:
```rust
let new_access_token = rand_utils::generate_random_str(16);
```

改为:
```rust
let new_access_token = rand_utils::generate_random_str(32);
```

- [ ] **Step 2: 修改 `refresh_access_token` 中的 token 生成**

将:
```rust
let new_access_token = rand_utils::generate_random_str(16);
```

改为:
```rust
let new_access_token = rand_utils::generate_random_str(32);
```

- [ ] **Step 3: 编译验证**

Run: `cargo build --features "auth"`
Expected: 编译成功，无 error

- [ ] **Step 4: Commit**

```bash
git add domains/auth/src/services/auth_service.rs
git commit -m "security(auth): 增加 access_token 长度至 128-bit (32 hex chars)"
```

---

### Task 5: 最终验证

- [ ] **Step 1: 完整构建验证**

Run: `cargo build --features "auth"`
Expected: 编译成功

- [ ] **Step 2: 运行测试**

Run: `cargo test --package auth`
Expected: 所有测试通过

- [ ] **Step 3: 检查无遗漏**

确认：
- `verify_email_verify_code` 返回 `Result<(), AppError>`
- 四个 service 函数都接收 `&AuthState`
- 注册成功后 Redis 中验证码被删除
- `generate_random_str` 调用均为 32
