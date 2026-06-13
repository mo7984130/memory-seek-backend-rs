# 测试策略设计

## 概述

为项目添加两层测试覆盖：
1. **单元测试** — 覆盖纯逻辑函数（无 IO 依赖）
2. **集成测试** — 覆盖完整 HTTP 请求链路（真实 DB + Redis + S3）

采用**混合策略**：不引入 mock 框架，不改动现有代码结构。纯逻辑用单元测试，IO 逻辑用集成测试。

## 现状

- 133 个源文件，12 个已有单元测试
- 0 个集成测试文件
- 已有测试基础设施描述（helpers/app、auth、db、minio），但代码未实现
- 测试基础设施：`tests/load/docker-compose.yml` 提供 PostgreSQL + Redis + MinIO

## 单元测试策略

### 覆盖范围

| 文件 | 测试重点 | 优先级 |
|------|----------|--------|
| `common/src/utils/token_cipher.rs` | 加密→解密往返、确定性 nonce、无效 token | P0（试点） |
| `common/src/utils/password_hash.rs` | 哈希→验证往返、不同密码不同哈希 | P1 |
| `server/src/middlewares/auth.rs` 的 `extract_bearer` | header 格式解析各种情况 | P1 |
| `common/src/ext/*.rs` | 扩展 trait 行为 | P2 |
| `common/src/r.rs` | `R<T>` 序列化格式 | P2 |
| `common/src/models/cursor_page.rs` | 游标分页逻辑 | P2 |
| `libs/img_url_generator/src/crypto.rs` | URL 签名/验证 | P2 |

### 测试模式

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name_scenario() {
        // Arrange
        // Act
        // Assert
    }
}
```

- 内联在源文件中，与现有风格一致
- 命名规范：`test_{功能}_{场景}`
- 使用 `assert!`、`assert_eq!`、`assert_matches!` 等标准断言

## 集成测试策略

### 文件结构

```
tests/
├── integration/
│   ├── main.rs              — 声明子模块
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── register.rs
│   │   ├── login.rs
│   │   └── token_refresh.rs
│   └── helpers/
│       ├── mod.rs
│       ├── app.rs           — build_test_router()
│       ├── auth.rs          — TestUser, register_user(), login_user()
│       ├── db.rs            — CleanupGuard
│       └── minio.rs         — MinIO 辅助
```

### 测试基础设施

**build_test_router()** — 构建包含所有已启用模块的 Router，使用**生产环境的 auth_middleware**。测试通过真实登录流程获取 token，后续请求通过 `Authorization: Bearer user_id token` header 传递认证信息。

**CleanupGuard** — 实现 `Drop` trait，测试结束时清理数据库中的测试数据。

**TestUser** — 测试用户结构体，包含 id、email、password、token 等字段。通过 `register_and_login()` 创建，返回已登录的用户实例。

**关键行为：** 用户登录时，该用户之前的所有 token 作废（单会话机制）。测试需覆盖此场景。

### Auth 模块测试用例

| 测试函数 | 描述 | 预期结果 |
|----------|------|----------|
| `test_register_success` | 正常注册 | 200，返回用户信息 |
| `test_register_duplicate_email` | 重复邮箱注册 | 409 Conflict |
| `test_register_invalid_password` | 密码不符合要求 | 400 Bad Request |
| `test_register_invalid_email` | 邮箱格式错误 | 400 Bad Request |
| `test_login_success` | 正常登录 | 200，返回 token |
| `test_login_wrong_password` | 错误密码 | 401 Unauthorized |
| `test_login_nonexistent_user` | 不存在的用户 | 401 Unauthorized |
| `test_token_refresh` | 刷新 token | 200，新旧 token 不同 |
| `test_login_invalidates_old_token` | 登录后旧 token 失效 | 用旧 token 请求返回 401 |

### 测试运行命令

```bash
# 单元测试
cargo test --lib

# 集成测试（需要先启动基础设施）
podman compose -f tests/load/docker-compose.yml up -d postgres redis minio
cargo test --test integration auth --features auth -- --test-threads=1
```

## 试点计划

### 第一步：单元测试 — token_cipher.rs

选择原因：纯逻辑、无 IO、加密解密是核心安全功能。

测试用例：
1. 加密→解密往返（相同 payload 恢复原值）
2. 确定性 nonce（相同 seed 产生相同密文）
3. 随机 nonce（无 seed 时每次密文不同）
4. 不同 key 解密失败
5. 无效 token 处理（太短、损坏、错误编码）

### 第二步：集成测试 — auth 模块

1. 实现 helpers（app.rs、auth.rs、db.rs）
2. 实现 register 测试（正常 + 错误场景）
3. 实现 login 测试（正常 + 错误场景）
4. 验证整个链路跑通

## 约定

- 集成测试必须串行运行（`--test-threads=1`）
- 每个测试结束前调用 `guard.cleanup().await`
- 所有请求体字段使用 camelCase
- 配置文件路径通过 `TEST_CONFIG_PATH` 环境变量指定
