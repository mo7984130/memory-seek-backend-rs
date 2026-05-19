# 函数级文档注释规范设计

## 概述

为项目全部 559 个函数添加完整的 Rust 文档注释，统一采用 `///` 格式，包含参数说明、返回值和错误说明。

## 目标

- 覆盖率从 28.1% 提升到 100%
- 统一文档风格，便于 IDE 提示和 `cargo doc` 生成
- 不修改函数内部逻辑，仅添加/重写文档注释

## 文档格式规范

### 基本格式

```rust
/// 函数功能简述
///
/// # 参数
/// - `param_name`: 参数说明
///
/// # 返回
/// 返回值说明
pub async fn function_name(param: Type) -> ReturnType {
```

### 完整格式（返回 Result 的函数）

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

### 特殊情况

- 无参数函数：省略 `# 参数` 部分
- 返回 `()` 或无意义返回：省略 `# 返回` 部分
- trait 实现方法：保留 trait 定义的文档，仅在缺失时补充
- 闭包和内部辅助函数：使用 `//` 行注释而非 `///`

## 分批处理计划

### 批次 1：domains/auth + domains/user

文件列表：
- `domains/auth/src/client/mod.rs`
- `domains/auth/src/client/token_store.rs`
- `domains/auth/src/config.rs`
- `domains/auth/src/controller/auth_controller.rs`
- `domains/auth/src/controller/mod.rs`
- `domains/auth/src/lib.rs`
- `domains/auth/src/models/mod.rs`
- `domains/auth/src/services/auth_service.rs`
- `domains/auth/src/services/mod.rs`
- `domains/auth/src/state.rs`
- `domains/user/src/client/mod.rs`
- `domains/user/src/config.rs`
- `domains/user/src/controller/mod.rs`
- `domains/user/src/controller/user_controller.rs`
- `domains/user/src/lib.rs`
- `domains/user/src/models/mod.rs`
- `domains/user/src/services/mod.rs`
- `domains/user/src/services/user_service.rs`
- `domains/user/src/state.rs`

### 批次 2：domains/photo (service/controller)

文件列表：
- `domains/photo/src/controller/collection_controller.rs`
- `domains/photo/src/controller/comment_controller.rs`
- `domains/photo/src/controller/face_controller.rs`
- `domains/photo/src/controller/mod.rs`
- `domains/photo/src/controller/photo_controller.rs`
- `domains/photo/src/controller/timeline_controller.rs`
- `domains/photo/src/services/collection_service.rs`
- `domains/photo/src/services/comment_service.rs`
- `domains/photo/src/services/face_service.rs`
- `domains/photo/src/services/feature_service.rs`
- `domains/photo/src/services/mod.rs`
- `domains/photo/src/services/photo_service.rs`
- `domains/photo/src/services/timeline_stat_service.rs`

### 批次 3：domains/photo (mapper/model) + entities

文件列表：
- `domains/photo/src/clustering/mod.rs`
- `domains/photo/src/clustering/union_find.rs`
- `domains/photo/src/clustering/vector_utils.rs`
- `domains/photo/src/mappers/*.rs` (8 files)
- `domains/photo/src/models/*.rs` (6 files)
- `domains/photo/src/state.rs`
- `domains/photo/src/utils/*.rs` (2 files)
- `entities/src/**/*.rs` (12 files)

### 批次 4：common

文件列表：
- `common/src/constants/**/*.rs` (6 files)
- `common/src/error/**/*.rs` (2 files)
- `common/src/extractors/*.rs` (3 files)
- `common/src/lib.rs`
- `common/src/macros/*.rs` (5 files)
- `common/src/models/*.rs` (3 files)
- `common/src/r.rs`
- `common/src/utils/*.rs` (15 files)

### 批次 5：server + libs

文件列表：
- `server/src/**/*.rs` (12 files)
- `libs/email/src/lib.rs`
- `libs/face_engine/src/*.rs` (6 files)
- `libs/img_url_generator/src/*.rs` (4 files)
- `libs/oss/src/lib.rs`

## 验证

每批次完成后执行：
```bash
cargo build --features "auth user photo metrics"
```

确保文档注释不影响编译。
