# Model Naming Unification Design

**Date:** 2026-06-12
**Status:** Approved
**Scope:** domains/auth, domains/user, domains/photo

## Overview

统一 domains 目录下所有模型的命名规范，消除 Request/Response/VO/DTO/Query 等多种后缀的混乱，统一使用 `Param` 和 `Result` 两种后缀。

## Naming Convention

### 核心规则

| 后缀 | 含义 | 替代的旧后缀 |
|------|------|-------------|
| `Param` | 输入参数（请求体、查询参数） | Request, Query, Param |
| `Result` | 输出结果（响应体、视图对象） | Response, VO, DTO |

### 例外情况

以下类型保持原命名不变：

- **辅助类型**：`PhotoCursor`, `CollectionPhotoCursor`, `TimeRange`, `PageDirection`
- **已符合规范的类型**：`Md5sExistParam`, `DeletePhotoParam`, `CommentPublishParam`, `CollectionUpdateParam`, `CollectionPhotoAddBatchParam`, `CollectionPhotoAddBatchResult`, `CollectionPhotoRemoveBatchParam`, `CollectionPhotoRemoveBatchResult`

### 特殊后缀

| 后缀 | 含义 | 使用场景 |
|------|------|---------|
| `Row` | 数据库查询结果 | 有 `FromQueryResult` 派生的类型（如 `UserInfoRow`） |

## Rename Plan

### Auth Module (4 types, 3 files)

| Old Name | New Name | File |
|----------|----------|------|
| `LoginRequest` | `LoginParam` | `domains/auth/src/models/mod.rs` |
| `RegisterRequest` | `RegisterParam` | `domains/auth/src/models/mod.rs` |
| `SendEmailCodeRequest` | `SendEmailCodeParam` | `domains/auth/src/models/mod.rs` |
| `AccessTokenResponse` | `AccessTokenResult` | `domains/auth/src/models/mod.rs` |

**Affected files:**
- `domains/auth/src/models/mod.rs`
- `domains/auth/src/services/auth_service.rs`
- `domains/auth/src/controller/auth_controller.rs`

### User Module (6 types, 3 files)

| Old Name | New Name | File |
|----------|----------|------|
| `ChangePasswordRequest` | `ChangePasswordParam` | `domains/user/src/models/mod.rs` |
| `ChangeNicknameRequest` | `ChangeNicknameParam` | `domains/user/src/models/mod.rs` |
| `GetUserInfoBatchRequest` | `GetUserInfoBatchParam` | `domains/user/src/models/mod.rs` |
| `InviterCodeDTO` | `InviterCodeResult` | `domains/user/src/models/mod.rs` |
| `UserInfoDTO` | `UserInfoRow` | `domains/user/src/models/mod.rs` |
| `UserInfoVO` | `UserInfoResult` | `domains/user/src/models/mod.rs` |

**Note:** `UserInfoDTO` 和 `UserInfoVO` 不能合并为同名类型，因为它们有不同的字段和用途：
- `UserInfoDTO` 是数据库查询结果（有 `FromQueryResult`），重命名为 `UserInfoRow`
- `UserInfoVO` 是 API 响应对象，重命名为 `UserInfoResult`

**Affected files:**
- `domains/user/src/models/mod.rs`
- `domains/user/src/services/user_service.rs`
- `domains/user/src/controller/user_controller.rs`

### Photo Module (7 types, 11 files)

| Old Name | New Name | File |
|----------|----------|------|
| `PhotoVO` | `PhotoResult` | `domains/photo/src/models/photo.rs` |
| `PhotoCursorQuery` | `PhotoCursorParam` | `domains/photo/src/models/photo.rs` |
| `PhotoCommentVO` | `PhotoCommentResult` | `domains/photo/src/models/comment.rs` |
| `CommentCursorPageQuery` | `CommentCursorPageParam` | `domains/photo/src/models/comment.rs` |
| `CollectionVO` | `CollectionResult` | `domains/photo/src/models/collection.rs` |
| `CollectionCreateParma` | `CollectionCreateParam` | `domains/photo/src/models/collection.rs` |
| `CollectionPhotoCursorPageQuery` | `CollectionPhotoCursorPageParam` | `domains/photo/src/models/collection.rs` |

**Note:** `CollectionCreateParma` 包含拼写错误（Parma → Param），一并修复。

**Affected files:**
- `domains/photo/src/models/photo.rs`
- `domains/photo/src/models/comment.rs`
- `domains/photo/src/models/collection.rs`
- `domains/photo/src/services/photo_service.rs`
- `domains/photo/src/services/collection_service.rs`
- `domains/photo/src/services/collection_photo_service.rs`
- `domains/photo/src/services/comment_service.rs`
- `domains/photo/src/controllers/photo_controller.rs`
- `domains/photo/src/controllers/collection_controller.rs`
- `domains/photo/src/controllers/collection_photo_controller.rs`
- `domains/photo/src/controllers/comment_controller.rs`

## Implementation Strategy

### Phase 1: Auth Module

1. Rename types in `domains/auth/src/models/mod.rs`
2. Update `domains/auth/src/services/auth_service.rs`
3. Update `domains/auth/src/controller/auth_controller.rs`
4. Run `cargo check --features auth` to verify

### Phase 2: User Module

1. Rename types in `domains/user/src/models/mod.rs`
2. Update `domains/user/src/services/user_service.rs`
3. Update `domains/user/src/controller/user_controller.rs`
4. Run `cargo check --features user` to verify

### Phase 3: Photo Module

1. Rename types in `domains/photo/src/models/photo.rs`
2. Rename types in `domains/photo/src/models/comment.rs`
3. Rename types in `domains/photo/src/models/collection.rs`
4. Update all service files
5. Update all controller files
6. Run `cargo check --features photo` to verify

### Phase 4: Final Verification

1. Run `cargo build --features "auth,user,photo"` to verify full build
2. Run `cargo test --lib` to verify unit tests
3. Run integration tests if available

## Risk Assessment

### Low Risk
- Simple rename operations
- No logic changes
- Compile-time verification

### Medium Risk
- Large number of files in Photo module (11 files)

### Mitigation
- Incremental implementation with compile checks
- Each module verified independently before proceeding

## Success Criteria

1. All model types follow `Param`/`Result` naming convention
2. No compilation errors
3. All existing tests pass
4. No functional changes to the application
