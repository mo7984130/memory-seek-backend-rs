# Metrics 宏重构方案

## 问题背景

当前 `metrics_group!()` 等宏使用 `module_path!()` 获取模块路径，生成的指标名格式为：

```
auth::services::login:duration
```

**问题**：`module_path!()` 返回完整的模块路径（如 `auth::services::login`），导致：
1. 指标名冗长，包含不必要的中间模块层级
2. 无法区分同一模块下的不同函数

## 目标格式

期望生成的指标名格式为：

```
{crate_name}:{function_name}:{metric_type}
```

示例：
- `auth:login:duration`
- `auth:login:concurrency`
- `auth:login:attempts`
- `auth:login:success`
- `auth:login:db_query:duration` (子操作计时)

## 技术方案

### 核心变更

使用 `env!("CARGO_PKG_NAME")` 替代 `module_path!()`：
- `env!("CARGO_PKG_NAME")` 在编译时获取 crate 名称（如 `auth`）
- 调用时必须显式传入函数名作为参数

### 修改的宏

#### 1. `metrics_group!`

```rust
// 修改前
metrics_group!();
metrics_group!("login");

// 修改后 - 函数名变为必填参数
metrics_group!("login");
```

#### 2. `metrics_success!`

```rust
// 修改前
metrics_success!();
metrics_success!("login");

// 修改后 - 函数名变为必填参数
metrics_success!("login");
```

#### 3. `metrics_timer_name!`

```rust
// 修改前
metrics_timer_name!("db_query")

// 修改后 - 函数名变为必填参数
metrics_timer_name!("login", "db_query")
```

#### 4. `timed!`

```rust
// 修改前
timed!("encrypt_avatar", expr)

// 修改后 - 函数名变为必填参数
timed!("login", "encrypt_avatar", expr)
```

## 修改文件清单

### Phase 1: 修改宏定义（common/src/macros/）

| 文件 | 修改内容 |
|------|----------|
| `metrics_group.rs` | 移除无参版本，所有版本第一个参数为函数名 |
| `metrics_success.rs` | 移除无参版本，所有版本第一个参数为函数名 |
| `metrics_timer_name.rs` | 移除无参版本，所有版本第一个参数为函数名 |
| `metrics_timed.rs` | 所有版本第一个参数为函数名 |

### Phase 2: 修改 auth 模块调用（domains/auth/src/services/）

| 文件 | 函数 | 当前调用 | 修改后 |
|------|------|----------|--------|
| `login.rs:42` | `login` | `metrics_group!()` | `metrics_group!("login")` |
| `login.rs:64` | `login` | `metrics_timer_name!("db_query")` | `metrics_timer_name!("login", "db_query")` |
| `login.rs:91` | `login` | `metrics_timer_name!("verify_password")` | `metrics_timer_name!("login", "verify_password")` |
| `login.rs:139` | `login` | `metrics_timer_name!("redis_set")` | `metrics_timer_name!("login", "redis_set")` |
| `login.rs:147` | `login` | `metrics_timer_name!("update_refresh_token")` | `metrics_timer_name!("login", "update_refresh_token")` |
| `login.rs:154` | `login` | `timed!("encrypt_avatar", ...)` | `timed!("login", "encrypt_avatar", ...)` |
| `login.rs:163` | `login` | `metrics_success!()` | `metrics_success!("login")` |
| `register.rs:34` | `register` | `metrics_group!()` | `metrics_group!("register")` |
| `register.rs:38` | `register` | `metrics_timer_name!("verify_email_code")` | `metrics_timer_name!("register", "verify_email_code")` |
| `register.rs:44` | `register` | `metrics_timer_name!("verify_inviter_code")` | `metrics_timer_name!("register", "verify_inviter_code")` |
| `register.rs:50` | `register` | `metrics_timer_name!("hash_password")` | `metrics_timer_name!("register", "hash_password")` |
| `register.rs:65` | `register` | `metrics_timer_name!("db_insert")` | `metrics_timer_name!("register", "db_insert")` |
| `register.rs:71` | `register` | `metrics_success!()` | `metrics_success!("register")` |
| `token.rs:25` | `refresh_access_token` | `metrics_group!()` | `metrics_group!("refresh_access_token")` |
| `token.rs:29` | `refresh_access_token` | `metrics_timer_name!("verify_token")` | `metrics_timer_name!("refresh_access_token", "verify_token")` |
| `token.rs:40` | `refresh_access_token` | `metrics_timer_name!("set_token")` | `metrics_timer_name!("refresh_access_token", "set_token")` |
| `token.rs:43` | `refresh_access_token` | `metrics_success!()` | `metrics_success!("refresh_access_token")` |
| `email.rs:28` | `send_email_code` | `metrics_group!()` | `metrics_group!("send_email_code")` |
| `email.rs:39` | `send_email_code` | `metrics_timer_name!("redis_set")` | `metrics_timer_name!("send_email_code", "redis_set")` |
| `email.rs:78` | `send_email_code` | `metrics_success!()` | `metrics_success!("send_email_code")` |

### Phase 3: 修改其他模块调用（后续）

| 模块 | 文件 | 需要修改的宏调用数量 |
|------|------|---------------------|
| `user` | `domains/user/src/services/mod.rs` | 12 处 |
| `photo` | `domains/photo/src/services/photo_service.rs` | 12 处 |

## 宏定义详细实现

### metrics_group.rs

```rust
#[macro_export]
macro_rules! metrics_group {
    // 单参数：函数名
    ($func:literal) => {
        #[cfg(feature = "metrics")]
        let _metrics_bundle = (
            $crate::utils::MetricsTimer::start(
                concat!(env!("CARGO_PKG_NAME"), ":", $func, ":duration")
            ),
            $crate::utils::MetricsConcurrencyGuard::start(
                concat!(env!("CARGO_PKG_NAME"), ":", $func, ":concurrency")
            ),
            $crate::metrics::counter!(
                concat!(env!("CARGO_PKG_NAME"), ":", $func, ":attempts")
            ).increment(1),
        );
    };
}
```

### metrics_success.rs

```rust
#[macro_export]
macro_rules! metrics_success {
    ($func:literal) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::counter!(
            concat!(env!("CARGO_PKG_NAME"), ":", $func, ":success")
        ).increment(1);
    };
}
```

### metrics_timer_name.rs

```rust
#[macro_export]
macro_rules! metrics_timer_name {
    ($func:literal, $name:literal) => {
        concat!(env!("CARGO_PKG_NAME"), ":", $func, ":", $name, ":duration")
    };
}
```

### metrics_timed.rs

```rust
#[macro_export]
macro_rules! timed {
    ($func:literal, $name:expr, $block:block) => {{
        #[cfg(feature = "metrics")]
        let _t = $crate::utils::MetricsTimer::start(
            concat!(env!("CARGO_PKG_NAME"), ":", $func, ":", $name, ":duration")
        );
        $block
    }};
    ($func:literal, $name:expr, $entry:expr) => {{
        #[cfg(feature = "metrics")]
        let _t = $crate::utils::MetricsTimer::start(
            concat!(env!("CARGO_PKG_NAME"), ":", $func, ":", $name, ":duration")
        );
        $entry
    }};
}
```

## 指标名生成示例

| 宏调用 | 生成的指标名 |
|--------|-------------|
| `metrics_group!("login")` | `auth:login:duration`, `auth:login:concurrency`, `auth:login:attempts` |
| `metrics_success!("login")` | `auth:login:success` |
| `metrics_timer_name!("login", "db_query")` | `auth:login:db_query:duration` |
| `timed!("login", "encrypt_avatar", expr)` | `auth:login:encrypt_avatar:duration` |

## 实施步骤

1. **Phase 1**: 修改 common/src/macros/ 下的 4 个宏定义文件
2. **Phase 2**: 修改 domains/auth/src/services/ 下的 4 个文件（login.rs, register.rs, token.rs, email.rs）
3. **Phase 3**: 编译验证 `cargo build --features "auth metrics"`
4. **Phase 4**: 后续修改 user 和 photo 模块

## 风险与注意事项

1. **Breaking Change**: 所有现有宏调用都需要修改，否则编译失败
2. **编译时验证**: `env!("CARGO_PKG_NAME")` 在编译时解析，如果 crate 不存在会编译失败
3. **函数名一致性**: 需要确保传入的函数名与实际函数名一致（建议通过代码审查保证）
