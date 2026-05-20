# ResultExt 命名清理与调用链优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 统一 ResultExt trait 的方法命名，修复仍在使用旧名称的调用点，并优化调用链避免连续重复的 trace_to_internal_err 调用。

**Architecture:** 基于 common::utils::ResultExt trait 提供的链式错误处理方法，确保所有调用点使用正确的方法名（trace_to_internal_err、trace_to_bad_request_warn、trace_conflict_warn），并通过 ChainErr 机制优化连续调用。

**Tech Stack:** Rust, tracing, common::utils::ResultExt

---

## 问题分析

### 1. 命名不一致问题

**旧名称（错误）：**
- `trace_bad_request_warn` → 应该是 `trace_to_bad_request_warn`
- `trace_internal_err` → 应该是 `trace_to_internal_err`
- `trace_conflict_err` → 应该是 `trace_conflict_warn`

**受影响的文件：**
- `domains/user/src/services/user_service.rs:227, 366, 379`
- `domains/photo/src/mappers/collection_photo_mapper.rs:199`

### 2. 文档注释中的旧名称

**文件：** `common/src/utils/result_ext.rs`
- 第 14 行：`trace_conflict_err` → `trace_conflict_warn`
- 第 69 行：`trace_internal_err` → `trace_to_internal_err`
- 第 75-76 行：示例代码中的旧方法名

### 3. 连续调用分析

**auth_service.rs:265-266** 的连续调用是合理的：
```rust
let hashed_pw = task::spawn_blocking(move || common::constants::HASHER.hash(&password_clone))
    .timed(metrics_timer_name!("register", "hash_password"))
    .await
    .trace_to_internal_err("spawn_blocking_error", "密码哈希任务执行失败")?  // 处理 JoinError
    .trace_to_internal_err("hash_password_error", "密码哈希计算失败")?;  // 处理哈希错误
```

这是处理 `Result<Result<String, E>, JoinError>` 的正确方式，不需要修改。

---

## 文件结构

### 需要修改的文件

- **Modify:** `domains/user/src/services/user_service.rs` — 修复 3 处旧方法名
- **Modify:** `domains/photo/src/mappers/collection_photo_mapper.rs` — 修复 1 处旧方法名
- **Modify:** `common/src/utils/result_ext.rs` — 更新文档注释中的旧方法名

### 无需修改的文件

- `domains/auth/src/services/auth_service.rs:265-266` — 连续调用是合理的，无需修改

---

## 任务分解

### Task 1: 修复 user_service.rs 中的旧方法名

**Files:**
- Modify: `domains/user/src/services/user_service.rs:227, 366, 379`

- [ ] **Step 1: 修复 trace_bad_request_warn 调用（第 227 行）**

将：
```rust
.trace_bad_request_warn("invalid_image", "文件验证失败")?
```
改为：
```rust
.trace_to_bad_request_warn("invalid_image", "文件验证失败")?
```

- [ ] **Step 2: 修复 trace_bad_request_warn 调用（第 366 行）**

将：
```rust
.trace_bad_request_warn("verify_error", "密码校验错误")?
```
改为：
```rust
.trace_to_bad_request_warn("verify_error", "密码校验错误")?
```

- [ ] **Step 3: 修复 trace_bad_request_warn 调用（第 379 行）**

将：
```rust
.trace_bad_request_warn("hash_error", "加密新密码失败")?
```
改为：
```rust
.trace_to_bad_request_warn("hash_error", "加密新密码失败")?
```

- [ ] **Step 4: 验证编译通过**

Run: `cargo build --features "user"`
Expected: 编译成功，无错误

- [ ] **Step 5: 运行测试**

Run: `cargo test --package user`
Expected: 所有测试通过

- [ ] **Step 6: 提交更改**

```bash
git add domains/user/src/services/user_service.rs
git commit -m "fix(user): rename trace_bad_request_warn to trace_to_bad_request_warn"
```

---

### Task 2: 修复 collection_photo_mapper.rs 中的旧方法名

**Files:**
- Modify: `domains/photo/src/mappers/collection_photo_mapper.rs:199`

- [ ] **Step 1: 修复 trace_internal_err 调用（第 199 行）**

将：
```rust
.trace_internal_err("db_insert_err", "添加到收藏夹失败")
```
改为：
```rust
.trace_to_internal_err("db_insert_err", "添加到收藏夹失败")
```

- [ ] **Step 2: 验证编译通过**

Run: `cargo build --features "photo"`
Expected: 编译成功，无错误

- [ ] **Step 3: 运行测试**

Run: `cargo test --package photo`
Expected: 所有测试通过

- [ ] **Step 4: 提交更改**

```bash
git add domains/photo/src/mappers/collection_photo_mapper.rs
git commit -m "fix(photo): rename trace_internal_err to trace_to_internal_err"
```

---

### Task 3: 更新 result_ext.rs 中的文档注释

**Files:**
- Modify: `common/src/utils/result_ext.rs:14, 69, 75-76`

- [ ] **Step 1: 更新第 14 行的注释**

将：
```rust
/// 用于 `trace_conflict_err` 等「部分处理」方法：
```
改为：
```rust
/// 用于 `trace_conflict_warn` 等「部分处理」方法：
```

- [ ] **Step 2: 更新第 69 行的注释**

将：
```rust
/// 允许后续继续链式调用 `trace_internal_err` 处理剩余错误。
```
改为：
```rust
/// 允许后续继续链式调用 `trace_to_internal_err` 处理剩余错误。
```

- [ ] **Step 3: 更新第 75-76 行的示例代码**

将：
```rust
///     .trace_conflict_err("db_insert_conflict", "照片已存在")?
///     .trace_internal_err("db_insert_err", "添加收藏夹失败")
```
改为：
```rust
///     .trace_conflict_warn("db_insert_conflict", "照片已存在")?
///     .trace_to_internal_err("db_insert_err", "添加收藏夹失败")
```

- [ ] **Step 4: 验证编译通过**

Run: `cargo build --features "auth user photo"`
Expected: 编译成功，无错误

- [ ] **Step 5: 提交更改**

```bash
git add common/src/utils/result_ext.rs
git commit -m "docs(common): update ResultExt trait documentation with correct method names"
```

---

### Task 4: 全量验证

- [ ] **Step 1: 搜索确认没有遗漏**

Run: `grep -rn "trace_bad_request_warn\|trace_internal_err\|trace_conflict_err" --include="*.rs" | grep -v "trace_to_bad_request_warn\|trace_to_internal_err\|trace_conflict_warn"`
Expected: 无输出，表示所有旧名称都已修复

- [ ] **Step 2: 完整构建验证**

Run: `cargo build`
Expected: 编译成功，无错误

- [ ] **Step 3: 完整测试验证**

Run: `cargo test`
Expected: 所有测试通过

- [ ] **Step 4: 最终提交（如果需要）**

```bash
git add -A
git commit -m "chore: complete ResultExt naming cleanup"
```

---

## 验证清单

### 命名一致性检查

- [ ] 所有 `trace_bad_request_warn` 已改为 `trace_to_bad_request_warn`
- [ ] 所有 `trace_internal_err` 已改为 `trace_to_internal_err`
- [ ] 所有 `trace_conflict_err` 已改为 `trace_conflict_warn`

### 调用链检查

- [ ] 确认 auth_service.rs:265-266 的连续调用是合理的（处理双层 Result）
- [ ] 无其他不必要的连续 trace_to_internal_err 调用

### 文档检查

- [ ] result_ext.rs 中的文档注释使用正确的方法名
- [ ] 示例代码使用正确的方法名

### 编译和测试检查

- [ ] `cargo build` 成功
- [ ] `cargo test` 全部通过

---

## 注意事项

1. **不要修改合理的连续调用**：auth_service.rs:265-266 的连续调用是处理 `Result<Result<T, E>, JoinError>` 的正确方式

2. **保持方法签名不变**：只修改方法名，不修改参数和返回值

3. **更新所有引用**：包括文档注释中的示例代码

4. **测试验证**：每个任务完成后都要运行测试确保功能正常

---

## 预期结果

完成此计划后：
1. ResultExt trait 的方法命名完全统一
2. 所有调用点使用正确的方法名
3. 文档注释与实际代码一致
4. 调用链清晰，无不必要的重复错误处理
