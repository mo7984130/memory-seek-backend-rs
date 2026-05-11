# User 模块全面优化设计

## 概述

对 `domains/user` 模块进行安全修复和代码质量优化，涵盖密码哈希统一、并发控制、敏感数据脱敏、缓存一致性、代码清理和纯函数测试。

## 问题清单

| # | 类型 | 问题 | 位置 |
|---|------|------|------|
| 1 | 安全 | `change_password` 使用 `bcrypt` 直接调用，与 auth 模块的 `HASHER`（argon2id）不一致 | `user_service.rs:282-303` |
| 2 | 安全 | 密码操作无并发控制，高并发下可能耗尽 CPU | `user_service.rs:282-303` |
| 3 | 安全 | `get_user_info` 返回完整 `UserDTO` 含 `refresh_token` 等敏感字段 | `user_service.rs:59-70` |
| 4 | 安全 | `logout` 未清除 `user_info_cache`，登出后缓存可能泄露旧数据 | `user_service.rs:331-358` |
| 5 | 质量 | `upload_avatar` 中 `new_key` 不必要的 clone | `user_service.rs:189` |
| 6 | 质量 | `UserInfoDTO` 有无用的 `Deserialize` derive | `models/mod.rs:42` |
| 7 | 质量 | `UserClient::get_user_info_batch_concurrent` 逐个请求而非用批量接口 | `client/mod.rs:212-230` |
| 8 | 质量 | `UserClient` 的 `reqwest::Client` 无超时配置 | `client/mod.rs:51` |
| 9 | 质量 | auth 和 user 模块有重复的头像加密逻辑 | 多处 |
| 10 | 质量 | `Duration::seconds` 在新版 chrono 中 deprecated | `user_service.rs:110` |

## 设计方案

### 1. 密码哈希统一

**文件：** `domains/user/src/services/user_service.rs`

将 `change_password` 中的：
```rust
use bcrypt::{hash, verify, DEFAULT_COST};
// verify(&req.old_password, &old_password)
// hash(password, DEFAULT_COST)
```
替换为：
```rust
use common::constants::HASHER;
// HASHER.verify(&req.old_password, &old_password)
// HASHER.hash(&password)
```

移除 `Cargo.toml` 中对 `bcrypt` 的直接依赖（如果其他地方不再使用）。

### 2. 密码操作并发控制

**文件：** `domains/user/src/services/user_service.rs`

添加与 auth 模块一致的信号量：
```rust
use std::sync::LazyLock;
use tokio::sync::Semaphore;

static PASSWORD_VERIFY_SEM: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(common::constants::get_password_verify_max_concurrency()));
```

在 `change_password` 的 verify 和 hash 操作前获取信号量。

### 3. 用户信息脱敏

**文件：** `domains/user/src/services/user_service.rs`

`get_user_info` 返回时将敏感字段设为 `None`：
```rust
Ok(user::UserDTO {
    id: user.id.to_string(),
    username: user.username,
    nickname: user.nickname,
    email: user.email,
    avatar_token,
    created_at: user.created_at.into(),
    refresh_token: None,
    refresh_token_expire_at: None,
    access_token: None,
    access_token_expire_at: None,
})
```

### 4. 登出清除缓存

**文件：** `domains/user/src/services/user_service.rs`

在 `logout` 函数的 `tokio::join!` 中增加第三个并发任务：
```rust
let (refresh_token_result, access_token_result, _) = tokio::join!(
    // ... 现有的 db update ...
    // ... 现有的 redis delete access_token ...
    redis.delete(&RedisKeys::user::user_info_cache(user_id))
        .timed(metrics_timer_name!("logout", "redis_delete_cache"))
);
```

### 5. 移除不必要的 clone

**文件：** `domains/user/src/services/user_service.rs`

移除 `new_key_for_db`，直接在闭包中使用 `new_key`：
```rust
let old_key = DbUtils::write(db, move |txn| {
    Box::pin(async move {
        // 直接使用 new_key（已 move 进来）
        ...
    })
})
```

### 6. 清理无用 derive

**文件：** `domains/user/src/models/mod.rs`

`UserInfoDTO` 移除 `Deserialize`：
```rust
#[derive(Serialize, FromQueryResult, Debug, Clone)]
pub struct UserInfoDTO { ... }
```

### 7. 改进 UserClient

**文件：** `domains/user/src/client/mod.rs`

- `reqwest::Client::builder().timeout(Duration::from_secs(30)).build()`
- `get_user_info_batch_concurrent` 改为调用 `get_user_info_batch` 批量接口

### 8. 提取头像加密逻辑

**文件：** `common/src/utils/mod.rs`（或新建 `common/src/utils/avatar.rs`）

提取公共函数：
```rust
pub fn encrypt_avatar_token(
    avatar_file_id: Option<&str>,
    token_cipher: &TokenCipher,
) -> Option<String> {
    avatar_file_id.and_then(|key| {
        token_cipher
            .encrypt(&ImageToken::thumbnail(key.to_string()), Some(key))
            .inspect_err(|e| warn!(error = %e, "加密头像失败"))
            .ok()
    })
}
```

在 auth `login`、user `get_user_info`、`update_avatar`、`UserInfoVO::from_dto` 中统一调用。

### 9. Duration 兼容性

**文件：** `domains/user/src/services/user_service.rs`

```rust
// 替换
Duration::seconds(INVITER_CODE_TTL_SECONDS)
// 为
TimeDelta::seconds(INVITER_CODE_TTL_SECONDS)
```

或使用 `Duration::try_seconds().unwrap()`。

### 10. 纯函数单元测试

**文件：** `domains/user/src/models/mod.rs`（`#[cfg(test)]` 模块）

测试：
- `UserInfoVO::from_dto` — 有头像 / 无头像 / 加密失败
- `ChangeNicknameRequest` 验证 — 正常 / 空 / 超长 / 特殊字符
- `ChangePasswordRequest` 验证 — 正常 / 无效密码

## 涉及文件

| 文件 | 改动类型 |
|------|----------|
| `domains/user/src/services/user_service.rs` | 安全修复 + 代码优化 |
| `domains/user/src/models/mod.rs` | 清理 derive + 添加测试 |
| `domains/user/src/client/mod.rs` | 改进 client |
| `domains/user/Cargo.toml` | 依赖调整 |
| `common/src/utils/mod.rs` 或新文件 | 提取头像加密函数 |

## 不做的事

- 不添加集成测试（用户明确不需要）
- 不重构模块结构
- 不修改 auth 模块（已有独立优化 spec）
- 不添加 rate limiting
