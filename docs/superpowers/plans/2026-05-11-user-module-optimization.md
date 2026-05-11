# User 模块全面优化实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 user 模块的安全漏洞（密码哈希不一致、并发控制缺失、敏感数据泄露、缓存不一致）并改善代码质量。

**Architecture:** 提取公共头像加密函数到 common，统一密码哈希为 argon2id，添加并发信号量，清理冗余代码，添加纯函数单元测试。

**Tech Stack:** Rust, sea-orm, deadpool-redis, bcrypt → argon2id, tokio, validator

---

## File Structure

| 文件 | 操作 | 职责 |
|------|------|------|
| `common/src/utils/avatar.rs` | 新建 | 公共头像加密函数 |
| `common/src/utils/mod.rs` | 修改 | 注册 avatar 模块并 re-export |
| `domains/user/src/services/user_service.rs` | 修改 | 安全修复 + 代码优化 |
| `domains/user/src/models/mod.rs` | 修改 | 清理 derive + 添加测试 |
| `domains/user/src/client/mod.rs` | 修改 | 改进 client |
| `domains/user/Cargo.toml` | 修改 | 移除 bcrypt，添加 tokio features |

---

### Task 1: 提取公共头像加密函数到 common

**Files:**
- Create: `common/src/utils/avatar.rs`
- Modify: `common/src/utils/mod.rs:10-11`（添加 mod + pub use）

- [ ] **Step 1: 创建 `common/src/utils/avatar.rs`**

```rust
use tracing::warn;

use crate::models::ImageToken;
use crate::utils::TokenCipher;

/// 加密头像 file_id 为 token
/// 返回 None 如果 file_id 为 None 或加密失败
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

- [ ] **Step 2: 在 `common/src/utils/mod.rs` 中注册模块**

在 `mod password;` 之后添加：

```rust
mod avatar;
```

在 `pub use password::{HashAlgorithm, Argon2idConfig, BcryptConfig};` 之后添加：

```rust
pub use avatar::encrypt_avatar_token;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p common`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add common/src/utils/avatar.rs common/src/utils/mod.rs
git commit -m "refactor(common): extract encrypt_avatar_token utility function"
```

---

### Task 2: 安全修复 — 密码哈希统一 + 并发控制

**Files:**
- Modify: `domains/user/src/services/user_service.rs:1-23`（imports）
- Modify: `domains/user/src/services/user_service.rs:257-323`（change_password）

- [ ] **Step 1: 修改 imports**

将 `user_service.rs` 第 1-23 行的 imports 替换为：

```rust
use chrono::{Duration, Utc};
use common::constants::RedisKeys;
use common::{metrics_group, metrics_success, metrics_timer_name, timed};
use deadpool_redis::Pool;
use entities::user;
use common::models::ImageToken;
use common::utils::TokenCipher;
use sea_orm::sea_query::Expr;
use sea_orm::sqlx::types::uuid;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
use tracing::{info, warn};
use tokio::task::spawn_blocking;
use std::sync::LazyLock;
use tokio::sync::Semaphore;

use crate::config::GET_USER_INFO_BATCH_MAX_LEN;
use crate::models::{ChangePasswordRequest, InviterCodeDTO, UserInfoDTO, UserInfoVO};
use common::constants::HASHER;
use common::error::AppError;
use common::utils::{DbUtils, MetricsTimerExt};
use common::utils::{rand_utils, FileValidator, encrypt_avatar_token};
use common::utils::{CacheExtension, RedisExt, ResultExt, OptionExt};
use oss::S3Client;

use crate::config::{GENERATE_INVITER_CODE_MAX_RETRY, INVITER_CODE_LEN, INVITER_CODE_TTL_SECONDS, USER_INFO_CACHE_TTL_SECS};

/// 密码验证并发信号量，限制同时进行的密码验证数量，防止 CPU 密集型操作抢占 runtime 资源
static PASSWORD_VERIFY_SEM: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(common::constants::get_password_verify_max_concurrency()));
```

- [ ] **Step 2: 替换 `change_password` 函数**

将 `user_service.rs` 中的 `change_password` 函数（约第 257-323 行）替换为：

```rust
/// 修改密码
#[tracing::instrument(
    name = "user_change_password",
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn change_password(
    db: &DatabaseConnection,
    redis: &Pool,
    user_id: i64,
    req: ChangePasswordRequest
) -> Result<(), AppError> {
    metrics_group!("change_password");

    // 新旧密码不可相同
    if req.old_password == req.new_password {
        return Err(AppError::bad_request("新密码不能与旧密码相同"))
    }

    //  获取旧密码
    let old_password: String = user::Entity::find_by_id(user_id)
        .select_only()
        .column(user::Column::Password)
        .into_tuple()
        .one(db)
        .timed(metrics_timer_name!("change_password", "db_query"))
        .await
        .trace_internal_err("db_query_error", "更改密码: 数据库查询用户失败")?
        .ok_or_warn("user_not_found", "更改密码", "用户不存在")?;

    // 效验旧密码（信号量保护，防止 CPU 耗尽）
    let is_valid = {
        let _permit = PASSWORD_VERIFY_SEM
            .acquire()
            .await
            .trace_internal_err("semaphore_error", "获取密码验证信号量失败")?;

        let password = req.old_password.clone();
        spawn_blocking(move || HASHER.verify(&password, &old_password))
            .timed(metrics_timer_name!("change_password", "verify_password"))
            .await
            .map_err(|_| AppError::InternalServerError)?
            .trace_bad_request_err("verify_error", "密码效验错误")?
    };
    if !is_valid {
        return Err(AppError::bad_request("原密码错误"));
    }

    // 加密新密码（信号量保护）
    let new_password_hash = {
        let _permit = PASSWORD_VERIFY_SEM
            .acquire()
            .await
            .trace_internal_err("semaphore_error", "获取密码验证信号量失败")?;

        let password = req.new_password.clone();
        spawn_blocking(move || HASHER.hash(&password))
            .timed(metrics_timer_name!("change_password", "hash_password"))
            .await
            .map_err(|_| AppError::InternalServerError)?
            .trace_bad_request_err("hash_error", "加密新密码失败")?
    };

    // 更新数据库
    user::ActiveModel {
        id: Set(user_id),
        password: Set(new_password_hash),
        ..Default::default()
    }
    .update(db)
    .timed(metrics_timer_name!("change_password", "db_update"))
    .await
    .trace_internal_err("db_update_error", "更改密码: 数据库更新错误")?;

    // 登出. 清除token
    logout(db, redis, user_id).await?;

    metrics_success!("change_password");
    info!(status = "success", user_id = %user_id, "修改密码成功");

    Ok(())
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p user`
Expected: 编译通过（bcrypt 仍被其他 import 引用，不会报错）

- [ ] **Step 4: Commit**

```bash
git add domains/user/src/services/user_service.rs
git commit -m "fix(user): use HASHER for password hashing, add concurrency semaphore"
```

---

### Task 3: 安全修复 — 用户信息脱敏 + 登出清除缓存

**Files:**
- Modify: `domains/user/src/services/user_service.rs:31-71`（get_user_info）
- Modify: `domains/user/src/services/user_service.rs:325-358`（logout）

- [ ] **Step 1: `get_user_info` 返回值脱敏**

将 `get_user_info` 函数末尾的 `Ok(user::UserDTO { ... })` 块（约第 59-70 行）替换为：

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

- [ ] **Step 2: `logout` 增加缓存清除**

将 `logout` 函数中的 `tokio::join!` 块（约第 340-351 行）替换为：

```rust
    let (refresh_token_result, access_token_result, _) = tokio::join!(
        user::ActiveModel {
            id: Set(user_id),
            refresh_token: Set(None),
            refresh_token_expire_at: Set(None),
            ..Default::default()
        }
            .update(db)
            .timed(metrics_timer_name!("logout", "db_update")),
        redis.delete(RedisKeys::user::user_access_token(user_id))
            .timed(metrics_timer_name!("logout", "redis_delete")),
        redis.delete(&RedisKeys::user::user_info_cache(user_id))
            .timed(metrics_timer_name!("logout", "redis_delete_cache"))
    );
    refresh_token_result.trace_internal_err("db_update_error", "登出时 清除refresh_token失败")?;
    access_token_result.trace_internal_err("redis_delete_error", "删除访问令牌失败")?;
```

注意：`_` 绑定的缓存删除错误不阻断登出流程（与现有模式一致）。

- [ ] **Step 3: 验证编译**

Run: `cargo check -p user`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add domains/user/src/services/user_service.rs
git commit -m "fix(user): sanitize get_user_info response, clear cache on logout"
```

---

### Task 4: 代码优化 — 移除冗余 clone + chrono 兼容 + 使用公共头像函数

**Files:**
- Modify: `domains/user/src/services/user_service.rs:47-54`（get_user_info 头像加密）
- Modify: `domains/user/src/services/user_service.rs:108-111`（generate_inviter_code chrono）
- Modify: `domains/user/src/services/user_service.rs:183-248`（upload_avatar）

- [ ] **Step 1: `get_user_info` 使用公共头像加密函数**

将 `get_user_info` 中的头像加密块（约第 47-54 行）替换为：

```rust
    let avatar_token = encrypt_avatar_token(user.avatar_file_id.as_deref(), token_cipher);
```

- [ ] **Step 2: `generate_inviter_code` chrono 兼容**

将第 110 行：
```rust
                expire_at: Utc::now() + Duration::seconds(INVITER_CODE_TTL_SECONDS)
```
替换为：
```rust
                expire_at: Utc::now() + Duration::try_seconds(INVITER_CODE_TTL_SECONDS).unwrap()
```

- [ ] **Step 3: `upload_avatar` 移除冗余 clone**

将 `upload_avatar` 中约第 189-212 行的：
```rust
    let new_key_for_db = new_key.clone();
    let old_key = DbUtils::write(db, move |txn| {
        let new_key_inner = new_key_for_db;

        Box::pin(async move {
            let old_key: Option<String> = user::Entity::find_by_id(user_id)
                .select_only()
                .column(user::Column::AvatarFileId)
                .into_values::<Option<String>, user::Column>()
                .one(txn)
                .await
                .trace_internal_err("db_query_error", "在上传头像时 查询头像url发生错误")?
                .ok_or_warn("user_not_found", "上传头像", "用户不存在")?;

            user::ActiveModel {
                id: Set(user_id),
                avatar_file_id: Set(Some(new_key_inner)),
                ..Default::default()
            }.update(txn).await
                .trace_internal_err("db_update_error", "在上传头像时 更新头像url发送错误")?;

            Ok(old_key)
        })
    })
```
替换为：
```rust
    let old_key = DbUtils::write(db, move |txn| {
        Box::pin(async move {
            let old_key: Option<String> = user::Entity::find_by_id(user_id)
                .select_only()
                .column(user::Column::AvatarFileId)
                .into_values::<Option<String>, user::Column>()
                .one(txn)
                .await
                .trace_internal_err("db_query_error", "在上传头像时 查询头像url发生错误")?
                .ok_or_warn("user_not_found", "上传头像", "用户不存在")?;

            user::ActiveModel {
                id: Set(user_id),
                avatar_file_id: Set(Some(new_key)),
                ..Default::default()
            }.update(txn).await
                .trace_internal_err("db_update_error", "在上传头像时 更新头像url发送错误")?;

            Ok(old_key)
        })
    })
```

- [ ] **Step 4: `upload_avatar` 末尾使用公共头像加密函数**

将 `upload_avatar` 末尾的头像 token 生成（约第 239-242 行）：
```rust
    let avatar_token = timed!("update_avatar", "encrypt_token",
        token_cipher.encrypt(&ImageToken::thumbnail(new_key.clone()), Some(&new_key))
            .trace_internal_err("encrypt_token_error", "生成头像token失败")?
    );
```
替换为：
```rust
    let avatar_token = encrypt_avatar_token(Some(&new_key), token_cipher)
        .ok_or_else(|| AppError::InternalServerError)?;
```

- [ ] **Step 5: 清理不再需要的 import**

移除 `use common::models::ImageToken;`（如果 `upload_avatar` 中不再直接使用）。检查 `get_user_info` 中是否还直接使用 `ImageToken` — 已改为用 `encrypt_avatar_token`，不再需要。

但注意 `upload_avatar` 中的 `FileValidator` 和 `ImageToken` 是否仍被使用。`FileValidator` 仍被使用。`ImageToken` 不再被 `user_service.rs` 直接使用，可以移除。

最终 import 区移除：
```rust
use common::models::ImageToken;
```

- [ ] **Step 6: 验证编译**

Run: `cargo check -p user`
Expected: 编译通过

- [ ] **Step 7: Commit**

```bash
git add domains/user/src/services/user_service.rs
git commit -m "refactor(user): remove redundant clone, use shared avatar encrypt, fix chrono deprecation"
```

---

### Task 5: 清理 models + 添加测试

**Files:**
- Modify: `domains/user/src/models/mod.rs:42`（移除 Deserialize）
- Modify: `domains/user/src/models/mod.rs`（添加 #[cfg(test)] 模块）

- [ ] **Step 1: 移除 `UserInfoDTO` 的 `Deserialize`**

将第 42 行：
```rust
#[derive(Serialize, FromQueryResult, Debug, Clone, Deserialize)]
```
替换为：
```rust
#[derive(Serialize, FromQueryResult, Debug, Clone)]
```

同时移除第 7 行不再需要的：
```rust
use serde::Deserialize;
```
（保留 `Serialize`，因为 `Serialize` 仍被使用。检查 `models/mod.rs` 中哪些 struct 用了 `Deserialize` — `ChangePasswordRequest`、`ChangeNicknameRequest`、`GetUserInfoBatchRequest` 仍需要 `Deserialize`，所以 import 不能移除。）

实际上 `Deserialize` 仍被其他 struct 使用，保留 import 不变。只修改 `UserInfoDTO` 的 derive。

- [ ] **Step 2: 添加测试模块**

在 `domains/user/src/models/mod.rs` 末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use common::utils::TokenCipher;
    use common::utils::token_cipher::TokenCipherConfig;
    use validator::Validate;

    fn create_test_cipher() -> TokenCipher {
        TokenCipher::new(TokenCipherConfig {
            key: "0123456789abcdef0123456789abcdef".to_string(),
        })
        .unwrap()
    }

    #[test]
    fn test_user_info_vo_from_dto_with_avatar() {
        let cipher = create_test_cipher();
        let dto = UserInfoDTO {
            user_id: 123,
            nickname: "test".to_string(),
            avatar_file_id: Some("avatars/123/test.jpg".to_string()),
        };
        let vo = UserInfoVO::from_dto(dto, &cipher);
        assert_eq!(vo.user_id, "123");
        assert_eq!(vo.nickname, "test");
        assert!(vo.avatar_token.is_some());
    }

    #[test]
    fn test_user_info_vo_from_dto_no_avatar() {
        let cipher = create_test_cipher();
        let dto = UserInfoDTO {
            user_id: 456,
            nickname: "noavatar".to_string(),
            avatar_file_id: None,
        };
        let vo = UserInfoVO::from_dto(dto, &cipher);
        assert_eq!(vo.user_id, "456");
        assert!(vo.avatar_token.is_none());
    }

    #[test]
    fn test_change_nickname_request_valid() {
        let req = ChangeNicknameRequest {
            new_nickname: "valid_name".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_change_nickname_request_empty() {
        let req = ChangeNicknameRequest {
            new_nickname: "".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_nickname_request_too_long() {
        let req = ChangeNicknameRequest {
            new_nickname: "a".repeat(21),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_nickname_request_special_chars() {
        let req = ChangeNicknameRequest {
            new_nickname: "test<script>".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_password_request_valid() {
        let req = ChangePasswordRequest {
            old_password: "oldPass123".to_string(),
            new_password: "newPass456".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_change_password_request_no_number() {
        let req = ChangePasswordRequest {
            old_password: "oldPassword".to_string(),
            new_password: "onlyLetters".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_change_password_request_too_short() {
        let req = ChangePasswordRequest {
            old_password: "oldPass123".to_string(),
            new_password: "a1".to_string(),
        };
        assert!(req.validate().is_err());
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `cargo test -p user`
Expected: 所有测试通过

- [ ] **Step 4: Commit**

```bash
git add domains/user/src/models/mod.rs
git commit -m "test(user): add unit tests for models, remove unused Deserialize from UserInfoDTO"
```

---

### Task 6: 改进 UserClient

**Files:**
- Modify: `domains/user/src/client/mod.rs:50-57`（构造函数）
- Modify: `domains/user/src/client/mod.rs:212-230`（get_user_info_batch_concurrent）

- [ ] **Step 1: 添加超时配置**

将 `UserClient::new`（约第 51-57 行）替换为：

```rust
    pub fn new(base_url: &str, auth_client: Arc<AuthClient>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            auth_client,
            base_url: base_url.to_string(),
        }
    }
```

- [ ] **Step 2: 改进 `get_user_info_batch_concurrent`**

将 `get_user_info_batch_concurrent` 函数（约第 212-230 行）替换为：

```rust
    /// 并发获取多个用户信息（使用批量接口）
    pub async fn get_user_info_batch_concurrent(
        &self,
        user_ids: &[i64],
        _concurrency: usize,
    ) -> Vec<Option<UserInfoVO>> {
        match self.get_user_info_batch(user_ids).await {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!(error = %e, "批量获取用户信息失败，回退到逐个请求");
                // 回退到逐个请求
                use futures::stream::{self, StreamExt};
                let results: Vec<Option<UserDTO>> = stream::iter(user_ids.iter())
                    .map(|&user_id| async move { self.get_user_info(user_id).await.ok() })
                    .buffer_unordered(_concurrency)
                    .collect()
                    .await;

                results.into_iter().map(|opt| opt.map(|user| UserInfoVO {
                    user_id: user.id,
                    nickname: user.nickname,
                    avatar_token: user.avatar_token,
                })).collect()
            }
        }
    }
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p user --features client`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add domains/user/src/client/mod.rs
git commit -m "refactor(user): add HTTP timeout, improve batch concurrent to use batch API"
```

---

### Task 7: 更新 Cargo.toml 依赖

**Files:**
- Modify: `domains/user/Cargo.toml`

- [ ] **Step 1: 移除 bcrypt 直接依赖**

在 `domains/user/Cargo.toml` 的 `[dependencies]` 中移除：
```toml
bcrypt = { workspace = true }
```

同时检查 `Cargo.toml` 是否有 `tokio` 的 `sync` feature。当前已有 `tokio = { workspace = true }`，需确认 workspace 中 tokio 启用了 `sync` feature。

- [ ] **Step 2: 验证编译**

Run: `cargo check -p user`
Expected: 编译通过

- [ ] **Step 3: 运行全部测试**

Run: `cargo test -p user`
Expected: 所有测试通过

- [ ] **Step 4: Commit**

```bash
git add domains/user/Cargo.toml
git commit -m "chore(user): remove direct bcrypt dependency"
```

---

### Task 8: 最终验证

- [ ] **Step 1: 全量编译检查**

Run: `cargo check`
Expected: 整个项目编译通过

- [ ] **Step 2: 运行 user 模块测试**

Run: `cargo test -p user`
Expected: 所有测试通过

- [ ] **Step 3: 运行 common 模块测试**

Run: `cargo test -p common`
Expected: 所有测试通过

- [ ] **Step 4: 确认无 bcrypt 残留引用**

Run: `grep -rn 'bcrypt' domains/user/src/`
Expected: 无输出
