# Auth 模块优化设计

日期: 2026-05-11

## 背景

审查 auth 模块代码后，发现安全性漏洞和代码质量问题需要修复。

## 修改范围

### 1. 安全性修复：注册后清理邮箱验证码

**问题**: `register` 函数成功注册用户后，Redis 中的邮箱验证码未被删除，攻击者可重复使用同一验证码注册多个账号。

**方案**: 在 `auth_service::register` 中，用户插入成功后，调用 Redis DEL 删除 `email_verify_code(&req.email)` 对应的 key。

**涉及文件**:
- `domains/auth/src/services/auth_service.rs` — `register` 函数

### 2. 代码质量：简化 controller-service 接口

**问题**: controller 每个函数手动拆解 `AuthState` 字段传给 service 函数，样板代码多，新增 state 字段时需同步改 service 签名。

**方案**: service 函数签名改为接收 `&AuthState` 引用，controller 只传 `&state`。

**涉及文件**:
- `domains/auth/src/services/auth_service.rs` — 所有 pub 函数签名
- `domains/auth/src/controller/auth_controller.rs` — 调用处

**变更示例**:
```rust
// Before
pub async fn login(db: &DatabaseConnection, redis: &Pool, req: LoginRequest, token_cipher: &TokenCipher) -> ...

// After
pub async fn login(state: &AuthState, req: LoginRequest) -> ...
```

### 3. 代码质量：修正 `verify_email_verify_code` 返回类型

**问题**: 返回 `Result<bool, AppError>`，调用方用 `.ok_or_warn()` 处理 bool，语义不直观。

**方案**: 改为 `Result<(), AppError>`，验证失败直接返回 `AppError::bad_request`。

**涉及文件**:
- `domains/auth/src/services/auth_service.rs` — `verify_email_verify_code` 函数及调用处

### 4. 代码质量：增加 access_token 长度

**问题**: `generate_random_str(16)` 生成 8 字节 (64-bit) hex token，安全性偏弱。

**方案**: 改为 `generate_random_str(32)` 生成 16 字节 (128-bit)。

**涉及文件**:
- `domains/auth/src/services/auth_service.rs` — `login` 和 `refresh_access_token` 中的 `generate_random_str` 调用

## 不修改的内容

- `refresh_access_token` 端点的认证方式（已有 refresh_token 验证）
- `verify_inviter_code` 中的硬编码后门（用户要求保留）
- `client` 模块（不在本次范围内）

## 验证方式

```bash
cargo build --features "auth"
cargo test --package auth
```
