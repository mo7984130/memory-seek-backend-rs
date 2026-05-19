# 函数级文档注释 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为项目全部 559 个函数添加完整的 Rust 文档注释，覆盖率从 28.1% 提升到 100%

**Architecture:** 按模块分 5 批处理，每批使用并行 Agent 处理多个文件，每批完成后验证 cargo build 通过

**Tech Stack:** Rust doc comments (`///`), cargo build

---

## 文档格式规范

所有函数统一采用以下格式：

```rust
/// 函数功能简述
///
/// # 参数
/// - `param_name`: 参数说明
///
/// # 返回
/// 返回值说明
///
/// # 错误
/// - `ErrorType`: 错误场景说明
pub async fn function_name(param: Type) -> Result<ReturnType, ErrorType> {
```

**特殊情况：**
- 无参数函数：省略 `# 参数`
- 返回 `()` 或无意义返回：省略 `# 返回`
- 返回 `Result` 的函数：必须包含 `# 错误`
- trait 实现方法：保留 trait 定义的文档，仅在缺失时补充
- 闭包和内部辅助函数：使用 `//` 行注释而非 `///`

---

## 批次 1：domains/auth + domains/user（85 个函数）

### Task 1.1: auth client 模块

**Files:**
- Modify: `domains/auth/src/client/mod.rs` (18 functions)
- Modify: `domains/auth/src/client/token_store.rs` (16 functions)

- [ ] **Step 1: 读取 domains/auth/src/client/mod.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/auth/src/client/token_store.rs，为所有函数添加文档注释**

- [ ] **Step 3: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 4: 提交**

```bash
git add domains/auth/src/client/
git commit -m "docs(auth): add doc comments to auth client module"
```

### Task 1.2: auth controller + services

**Files:**
- Modify: `domains/auth/src/controller/auth_controller.rs` (5 functions)
- Modify: `domains/auth/src/services/auth_service.rs` (7 functions)
- Modify: `domains/auth/src/state.rs` (1 function)

- [ ] **Step 1: 读取 domains/auth/src/controller/auth_controller.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/auth/src/services/auth_service.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 domains/auth/src/state.rs，为所有函数添加文档注释**

- [ ] **Step 4: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 5: 提交**

```bash
git add domains/auth/src/controller/ domains/auth/src/services/ domains/auth/src/state.rs
git commit -m "docs(auth): add doc comments to auth controller and services"
```

### Task 1.3: auth 剩余文件

**Files:**
- Modify: `domains/auth/src/config.rs` (0 functions, module-level doc)
- Modify: `domains/auth/src/lib.rs` (0 functions, module-level doc)
- Modify: `domains/auth/src/models/mod.rs` (0 functions, module-level doc)
- Modify: `domains/auth/src/services/mod.rs` (0 functions, module-level doc)
- Modify: `domains/auth/src/controller/mod.rs` (0 functions, module-level doc)

- [ ] **Step 1: 检查这些文件是否有需要文档化的函数，如有则添加**

- [ ] **Step 2: 提交**

```bash
git add domains/auth/
git commit -m "docs(auth): add remaining auth module docs"
```

### Task 1.4: user client 模块

**Files:**
- Modify: `domains/user/src/client/mod.rs` (11 functions)

- [ ] **Step 1: 读取 domains/user/src/client/mod.rs，为所有函数添加文档注释**

- [ ] **Step 2: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 3: 提交**

```bash
git add domains/user/src/client/
git commit -m "docs(user): add doc comments to user client module"
```

### Task 1.5: user controller + services

**Files:**
- Modify: `domains/user/src/controller/user_controller.rs` (8 functions)
- Modify: `domains/user/src/services/user_service.rs` (7 functions)
- Modify: `domains/user/src/state.rs` (1 function)

- [ ] **Step 1: 读取 domains/user/src/controller/user_controller.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/user/src/services/user_service.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 domains/user/src/state.rs，为所有函数添加文档注释**

- [ ] **Step 4: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 5: 提交**

```bash
git add domains/user/src/controller/ domains/user/src/services/ domains/user/src/state.rs
git commit -m "docs(user): add doc comments to user controller and services"
```

### Task 1.6: user models + 剩余文件

**Files:**
- Modify: `domains/user/src/models/mod.rs` (11 functions)
- Modify: `domains/user/src/config.rs` (0 functions, module-level doc)
- Modify: `domains/user/src/lib.rs` (0 functions, module-level doc)
- Modify: `domains/user/src/services/mod.rs` (0 functions, module-level doc)
- Modify: `domains/user/src/controller/mod.rs` (0 functions, module-level doc)

- [ ] **Step 1: 读取 domains/user/src/models/mod.rs，为所有函数添加文档注释**

- [ ] **Step 2: 检查剩余文件是否有需要文档化的函数**

- [ ] **Step 3: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 4: 提交**

```bash
git add domains/user/
git commit -m "docs(user): add doc comments to user models and remaining files"
```

---

## 批次 2：domains/photo service/controller（82 个函数）

### Task 2.1: photo controller 模块

**Files:**
- Modify: `domains/photo/src/controller/photo_controller.rs` (15 functions)
- Modify: `domains/photo/src/controller/collection_controller.rs` (11 functions)
- Modify: `domains/photo/src/controller/face_controller.rs` (11 functions)

- [ ] **Step 1: 读取 domains/photo/src/controller/photo_controller.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/photo/src/controller/collection_controller.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 domains/photo/src/controller/face_controller.rs，为所有函数添加文档注释**

- [ ] **Step 4: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 5: 提交**

```bash
git add domains/photo/src/controller/
git commit -m "docs(photo): add doc comments to photo controllers"
```

### Task 2.2: photo controller 剩余 + comment/timeline

**Files:**
- Modify: `domains/photo/src/controller/comment_controller.rs` (5 functions)
- Modify: `domains/photo/src/controller/timeline_controller.rs` (2 functions)
- Modify: `domains/photo/src/controller/mod.rs` (0 functions, module-level doc)

- [ ] **Step 1: 读取 domains/photo/src/controller/comment_controller.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/photo/src/controller/timeline_controller.rs，为所有函数添加文档注释**

- [ ] **Step 3: 检查 mod.rs 是否有需要文档化的函数**

- [ ] **Step 4: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 5: 提交**

```bash
git add domains/photo/src/controller/
git commit -m "docs(photo): add doc comments to comment and timeline controllers"
```

### Task 2.3: photo services 核心

**Files:**
- Modify: `domains/photo/src/services/photo_service.rs` (6 functions)
- Modify: `domains/photo/src/services/collection_service.rs` (12 functions)
- Modify: `domains/photo/src/services/comment_service.rs` (4 functions)

- [ ] **Step 1: 读取 domains/photo/src/services/photo_service.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/photo/src/services/collection_service.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 domains/photo/src/services/comment_service.rs，为所有函数添加文档注释**

- [ ] **Step 4: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 5: 提交**

```bash
git add domains/photo/src/services/
git commit -m "docs(photo): add doc comments to photo, collection, comment services"
```

### Task 2.4: photo services 人脸/特征/时间线

**Files:**
- Modify: `domains/photo/src/services/face_service.rs` (13 functions)
- Modify: `domains/photo/src/services/feature_service.rs` (8 functions)
- Modify: `domains/photo/src/services/timeline_stat_service.rs` (5 functions)
- Modify: `domains/photo/src/services/mod.rs` (0 functions, module-level doc)

- [ ] **Step 1: 读取 domains/photo/src/services/face_service.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/photo/src/services/feature_service.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 domains/photo/src/services/timeline_stat_service.rs，为所有函数添加文档注释**

- [ ] **Step 4: 检查 mod.rs 是否有需要文档化的函数**

- [ ] **Step 5: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 6: 提交**

```bash
git add domains/photo/src/services/
git commit -m "docs(photo): add doc comments to face, feature, timeline services"
```

---

## 批次 3：domains/photo mapper/model + entities（123 个函数）

### Task 3.1: photo mappers 核心

**Files:**
- Modify: `domains/photo/src/mappers/photo_mapper.rs` (10 functions)
- Modify: `domains/photo/src/mappers/collection_mapper.rs` (9 functions)
- Modify: `domains/photo/src/mappers/collection_photo_mapper.rs` (11 functions)
- Modify: `domains/photo/src/mappers/comment_mapper.rs` (10 functions)

- [ ] **Step 1: 读取 domains/photo/src/mappers/photo_mapper.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/photo/src/mappers/collection_mapper.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 domains/photo/src/mappers/collection_photo_mapper.rs，为所有函数添加文档注释**

- [ ] **Step 4: 读取 domains/photo/src/mappers/comment_mapper.rs，为所有函数添加文档注释**

- [ ] **Step 5: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 6: 提交**

```bash
git add domains/photo/src/mappers/
git commit -m "docs(photo): add doc comments to photo, collection, comment mappers"
```

### Task 3.2: photo mappers 人脸/评论点赞/时间线

**Files:**
- Modify: `domains/photo/src/mappers/face_feature_mapper.rs` (12 functions)
- Modify: `domains/photo/src/mappers/face_person_mapper.rs` (11 functions)
- Modify: `domains/photo/src/mappers/comment_like_mapper.rs` (6 functions)
- Modify: `domains/photo/src/mappers/timeline_stat_mapper.rs` (1 function)
- Modify: `domains/photo/src/mappers/mod.rs` (0 functions, module-level doc)

- [ ] **Step 1: 读取 domains/photo/src/mappers/face_feature_mapper.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/photo/src/mappers/face_person_mapper.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 domains/photo/src/mappers/comment_like_mapper.rs，为所有函数添加文档注释**

- [ ] **Step 4: 读取 domains/photo/src/mappers/timeline_stat_mapper.rs，为所有函数添加文档注释**

- [ ] **Step 5: 检查 mod.rs 是否有需要文档化的函数**

- [ ] **Step 6: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 7: 提交**

```bash
git add domains/photo/src/mappers/
git commit -m "docs(photo): add doc comments to face, comment_like, timeline mappers"
```

### Task 3.3: photo models + clustering + utils

**Files:**
- Modify: `domains/photo/src/models/photo.rs` (8 functions)
- Modify: `domains/photo/src/models/collection.rs` (2 functions)
- Modify: `domains/photo/src/models/face.rs` (2 functions)
- Modify: `domains/photo/src/clustering/union_find.rs` (11 functions)
- Modify: `domains/photo/src/clustering/vector_utils.rs` (16 functions)
- Modify: `domains/photo/src/utils/pinyin.rs` (4 functions)
- Modify: `domains/photo/src/state.rs` (1 function)

- [ ] **Step 1: 读取 domains/photo/src/models/photo.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 domains/photo/src/models/collection.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 domains/photo/src/models/face.rs，为所有函数添加文档注释**

- [ ] **Step 4: 读取 domains/photo/src/clustering/union_find.rs，为所有函数添加文档注释**

- [ ] **Step 5: 读取 domains/photo/src/clustering/vector_utils.rs，为所有函数添加文档注释**

- [ ] **Step 6: 读取 domains/photo/src/utils/pinyin.rs，为所有函数添加文档注释**

- [ ] **Step 7: 读取 domains/photo/src/state.rs，为所有函数添加文档注释**

- [ ] **Step 8: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 9: 提交**

```bash
git add domains/photo/src/models/ domains/photo/src/clustering/ domains/photo/src/utils/ domains/photo/src/state.rs
git commit -m "docs(photo): add doc comments to models, clustering, utils"
```

### Task 3.4: entities 模块

**Files:**
- Modify: `entities/src/vector.rs` (17 functions)
- Modify: `entities/src/photo_entities/photo.rs` (3 functions)
- Modify: `entities/src/photo_entities/collection.rs` (1 function)
- Modify: `entities/src/photo_entities/collection_photo.rs` (2 functions)
- Modify: `entities/src/photo_entities/comment.rs` (2 functions)
- Modify: `entities/src/photo_entities/comment_like.rs` (1 function)
- Modify: `entities/src/photo_entities/face_feature.rs` (2 functions)
- Modify: `entities/src/photo_entities/face_person.rs` (1 function)
- Modify: `entities/src/user_entities/user.rs` (2 functions)
- Modify: `entities/src/lib.rs` (0 functions, module-level doc)

- [ ] **Step 1: 读取 entities/src/vector.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 entities/src/photo_entities/ 下所有文件，为函数添加文档注释**

- [ ] **Step 3: 读取 entities/src/user_entities/user.rs，为所有函数添加文档注释**

- [ ] **Step 4: 检查 lib.rs 是否有需要文档化的函数**

- [ ] **Step 5: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 6: 提交**

```bash
git add entities/
git commit -m "docs(entities): add doc comments to all entity files"
```

---

## 批次 4：common（151 个函数）

### Task 4.1: common error + extractors + models

**Files:**
- Modify: `common/src/error/app_error.rs` (6 functions)
- Modify: `common/src/error/mod.rs` (0 functions, module-level doc)
- Modify: `common/src/extractors/client_ip.rs` (1 function)
- Modify: `common/src/extractors/validated_json.rs` (2 functions)
- Modify: `common/src/extractors/mod.rs` (0 functions, module-level doc)
- Modify: `common/src/models/image_token.rs` (4 functions)
- Modify: `common/src/models/user_id.rs` (0 functions, module-level doc)
- Modify: `common/src/models/mod.rs` (0 functions, module-level doc)
- Modify: `common/src/r.rs` (3 functions)

- [ ] **Step 1: 读取 common/src/error/app_error.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 common/src/extractors/ 下所有文件，为函数添加文档注释**

- [ ] **Step 3: 读取 common/src/models/ 下所有文件，为函数添加文档注释**

- [ ] **Step 4: 读取 common/src/r.rs，为所有函数添加文档注释**

- [ ] **Step 5: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 6: 提交**

```bash
git add common/src/error/ common/src/extractors/ common/src/models/ common/src/r.rs
git commit -m "docs(common): add doc comments to error, extractors, models, r"
```

### Task 4.2: common utils 核心

**Files:**
- Modify: `common/src/utils/password.rs` (6 functions)
- Modify: `common/src/utils/token_cipher.rs` (6 functions)
- Modify: `common/src/utils/redis_ext.rs` (12 functions)
- Modify: `common/src/utils/db_utils.rs` (1 function)
- Modify: `common/src/utils/avatar.rs` (1 function)

- [ ] **Step 1: 读取 common/src/utils/password.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 common/src/utils/token_cipher.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 common/src/utils/redis_ext.rs，为所有函数添加文档注释**

- [ ] **Step 4: 读取 common/src/utils/db_utils.rs，为所有函数添加文档注释**

- [ ] **Step 5: 读取 common/src/utils/avatar.rs，为所有函数添加文档注释**

- [ ] **Step 6: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 7: 提交**

```bash
git add common/src/utils/
git commit -m "docs(common): add doc comments to core utils"
```

### Task 4.3: common utils 扩展 + 验证器

**Files:**
- Modify: `common/src/utils/result_ext.rs` (26 functions)
- Modify: `common/src/utils/option_ext.rs` (4 functions)
- Modify: `common/src/utils/bool_ext.rs` (4 functions)
- Modify: `common/src/utils/rand_utils.rs` (11 functions)
- Modify: `common/src/utils/file_validator.rs` (20 functions)

- [ ] **Step 1: 读取 common/src/utils/result_ext.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 common/src/utils/option_ext.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 common/src/utils/bool_ext.rs，为所有函数添加文档注释**

- [ ] **Step 4: 读取 common/src/utils/rand_utils.rs，为所有函数添加文档注释**

- [ ] **Step 5: 读取 common/src/utils/file_validator.rs，为所有函数添加文档注释**

- [ ] **Step 6: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 7: 提交**

```bash
git add common/src/utils/
git commit -m "docs(common): add doc comments to result_ext, option_ext, bool_ext, rand_utils, file_validator"
```

### Task 4.4: common validators + metrics + constants

**Files:**
- Modify: `common/src/utils/validators/account.rs` (6 functions)
- Modify: `common/src/utils/validators/email.rs` (4 functions)
- Modify: `common/src/utils/validators/normal_chars.rs` (5 functions)
- Modify: `common/src/utils/validators/password.rs` (8 functions)
- Modify: `common/src/utils/validators/username.rs` (5 functions)
- Modify: `common/src/utils/validators/mod.rs` (0 functions, module-level doc)
- Modify: `common/src/utils/metrics_ext/metrics_concurrency_guard.rs` (2 functions)
- Modify: `common/src/utils/metrics_ext/metrics_timer.rs` (2 functions)
- Modify: `common/src/utils/metrics_ext/metrics_timer_ext.rs` (1 function)
- Modify: `common/src/utils/metrics_ext/mod.rs` (0 functions, module-level doc)
- Modify: `common/src/utils/mod.rs` (0 functions, module-level doc)

- [ ] **Step 1: 读取 common/src/utils/validators/ 下所有文件，为函数添加文档注释**

- [ ] **Step 2: 读取 common/src/utils/metrics_ext/ 下所有文件，为函数添加文档注释**

- [ ] **Step 3: 检查 mod.rs 文件是否有需要文档化的函数**

- [ ] **Step 4: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 5: 提交**

```bash
git add common/src/utils/validators/ common/src/utils/metrics_ext/
git commit -m "docs(common): add doc comments to validators and metrics_ext"
```

### Task 4.5: common constants + macros + lib

**Files:**
- Modify: `common/src/constants/password_concurrency.rs` (1 function)
- Modify: `common/src/constants/password_hasher.rs` (1 function)
- Modify: `common/src/constants/redis_keys/photo.rs` (4 functions)
- Modify: `common/src/constants/redis_keys/user.rs` (5 functions)
- Modify: `common/src/constants/redis_keys/mod.rs` (0 functions, module-level doc)
- Modify: `common/src/constants/mod.rs` (0 functions, module-level doc)
- Modify: `common/src/macros/metrics_group.rs` (0 functions, macro-level doc)
- Modify: `common/src/macros/metrics_success.rs` (0 functions, macro-level doc)
- Modify: `common/src/macros/metrics_timed.rs` (0 functions, macro-level doc)
- Modify: `common/src/macros/metrics_timer_name.rs` (0 functions, macro-level doc)
- Modify: `common/src/macros/mod.rs` (0 functions, module-level doc)
- Modify: `common/src/lib.rs` (0 functions, module-level doc)

- [ ] **Step 1: 读取 common/src/constants/ 下所有文件，为函数添加文档注释**

- [ ] **Step 2: 读取 common/src/macros/ 下所有文件，为宏添加文档注释**

- [ ] **Step 3: 检查 lib.rs 是否有需要文档化的函数**

- [ ] **Step 4: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 5: 提交**

```bash
git add common/
git commit -m "docs(common): add doc comments to constants, macros, lib"
```

---

## 批次 5：server + libs（86 个函数）

### Task 5.1: server 模块

**Files:**
- Modify: `server/src/config.rs` (2 functions)
- Modify: `server/src/main.rs` (2 functions)
- Modify: `server/src/metrics.rs` (4 functions)
- Modify: `server/src/middlewares/auth.rs` (1 function)
- Modify: `server/src/middlewares/trace_id.rs` (1 function)
- Modify: `server/src/middlewares/mod.rs` (0 functions, module-level doc)
- Modify: `server/src/setup/auth.rs` (2 functions)
- Modify: `server/src/setup/database.rs` (1 function)
- Modify: `server/src/setup/log.rs` (1 function)
- Modify: `server/src/setup/photo.rs` (3 functions)
- Modify: `server/src/setup/redis.rs` (1 function)
- Modify: `server/src/setup/user.rs` (2 functions)
- Modify: `server/src/setup/mod.rs` (0 functions, module-level doc)
- Modify: `server/src/state.rs` (0 functions, module-level doc)
- Modify: `server/src/utils/mod.rs` (0 functions, module-level doc)

- [ ] **Step 1: 读取 server/src/config.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 server/src/main.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 server/src/metrics.rs，为所有函数添加文档注释**

- [ ] **Step 4: 读取 server/src/middlewares/ 下所有文件，为函数添加文档注释**

- [ ] **Step 5: 读取 server/src/setup/ 下所有文件，为函数添加文档注释**

- [ ] **Step 6: 检查剩余文件是否有需要文档化的函数**

- [ ] **Step 7: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 8: 提交**

```bash
git add server/
git commit -m "docs(server): add doc comments to all server modules"
```

### Task 5.2: libs/face_engine

**Files:**
- Modify: `libs/face_engine/src/aligner.rs` (6 functions)
- Modify: `libs/face_engine/src/base.rs` (5 functions)
- Modify: `libs/face_engine/src/detector.rs` (5 functions)
- Modify: `libs/face_engine/src/lazy_engine.rs` (8 functions)
- Modify: `libs/face_engine/src/lib.rs` (4 functions)
- Modify: `libs/face_engine/src/recognizer.rs` (4 functions)
- Modify: `libs/face_engine/src/types.rs` (4 functions)

- [ ] **Step 1: 读取 libs/face_engine/src/aligner.rs，为所有函数添加文档注释**

- [ ] **Step 2: 读取 libs/face_engine/src/base.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 libs/face_engine/src/detector.rs，为所有函数添加文档注释**

- [ ] **Step 4: 读取 libs/face_engine/src/lazy_engine.rs，为所有函数添加文档注释**

- [ ] **Step 5: 读取 libs/face_engine/src/lib.rs，为所有函数添加文档注释**

- [ ] **Step 6: 读取 libs/face_engine/src/recognizer.rs，为所有函数添加文档注释**

- [ ] **Step 7: 读取 libs/face_engine/src/types.rs，为所有函数添加文档注释**

- [ ] **Step 8: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 9: 提交**

```bash
git add libs/face_engine/
git commit -m "docs(face_engine): add doc comments to all face_engine files"
```

### Task 5.3: libs/img_url_generator + email + oss

**Files:**
- Modify: `libs/img_url_generator/src/alioss_generator.rs` (4 functions)
- Modify: `libs/img_url_generator/src/imgproxy_generator.rs` (6 functions)
- Modify: `libs/img_url_generator/src/lib.rs` (9 functions)
- Modify: `libs/email/src/lib.rs` (2 functions)
- Modify: `libs/oss/src/lib.rs` (9 functions)

- [ ] **Step 1: 读取 libs/img_url_generator/src/ 下所有文件，为函数添加文档注释**

- [ ] **Step 2: 读取 libs/email/src/lib.rs，为所有函数添加文档注释**

- [ ] **Step 3: 读取 libs/oss/src/lib.rs，为所有函数添加文档注释**

- [ ] **Step 4: 验证编译**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 5: 提交**

```bash
git add libs/
git commit -m "docs(libs): add doc comments to img_url_generator, email, oss"
```

---

## 最终验证

- [ ] **Step 1: 完整编译验证**

```bash
cargo build --features "auth user photo metrics"
```

- [ ] **Step 2: 生成文档验证**

```bash
cargo doc --no-deps --features "auth user photo metrics"
```

- [ ] **Step 3: 统计最终覆盖率**

```bash
python3 -c "
import re, os

total_fns = 0
documented = 0

for root, dirs, files in os.walk('.'):
    if 'target' in root:
        continue
    for f in files:
        if not f.endswith('.rs'):
            continue
        path = os.path.join(root, f)
        with open(path) as fh:
            lines = fh.readlines()
        for i, line in enumerate(lines):
            if re.match(r'^\s*(pub\s+)?(async\s+)?fn\s+', line):
                total_fns += 1
                j = i - 1
                while j >= 0 and lines[j].strip() == '':
                    j -= 1
                if j >= 0 and lines[j].strip().startswith('///'):
                    documented += 1

print(f'Total: {total_fns}')
print(f'Documented: {documented}')
print(f'Coverage: {documented/total_fns*100:.1f}%')
"
```

预期输出：Coverage: 100.0%
