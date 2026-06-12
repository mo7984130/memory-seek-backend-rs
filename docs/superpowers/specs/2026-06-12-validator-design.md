# 控制器参数校验设计方案

## 概述

为所有 Controller 添加统一的参数校验机制，使用 `validator` crate 的 `Validate` derive 宏在模型层定义校验规则，通过自定义提取器在请求进入控制器前自动执行校验。

## 背景

### 现状

- 项目已有 `validator = "0.20.0"` 依赖（workspace 级别）
- `common` 模块已有 `ValidatedJson` 提取器，支持 JSON 请求体的自动校验
- Auth 和 User 模块已使用 `#[derive(Validate)]` + `ValidatedJson` 模式
- Photo 模块的控制器直接使用 `Json` 和 `Query`，**未做参数校验**

### 问题

1. Photo 模块 6 个 JSON 请求模型缺少校验
2. 3 个 Query 参数模型缺少校验
3. 没有 `ValidatedQuery` 提取器

## 设计方案

### 1. 新增 `ValidatedQuery` 提取器

**文件**: `common/src/extractors/validated_query.rs`

仿照 `ValidatedJson` 的实现模式，新增 `ValidatedQuery` 提取器：

```rust
use axum::extract::{FromRequestParts, Query};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::Validate;

use crate::{error::AppError, ext::{ResultErrExt, log_warn}};

/// 带自动验证的 Query 参数提取器
pub struct ValidatedQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .trace_warn(
                "validated_query_parse_err",
                "解析查询参数失败",
                AppError::bad_request("解析查询参数失败"),
            )?;

        value.validate().map_err(|err| {
            let msg = err
                .field_errors()
                .into_iter()
                .map(|(field, errors)| {
                    let messages: Vec<String> = errors
                        .iter()
                        .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
                        .collect();
                    format!("{}: {}", field, messages.join(", "))
                })
                .collect::<Vec<_>>()
                .join("; ");
            log_warn(
                "validated_query_validate_err",
                "效验失败",
                err,
                AppError::bad_request(msg),
            )
        })?;

        Ok(ValidatedQuery(value))
    }
}

impl<T> Deref for ValidatedQuery<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
```

**注册**: 在 `common/src/extractors/mod.rs` 中导出。

### 2. Photo 模块模型校验规则

#### 2.1 Collection 相关模型

**文件**: `domains/photo/src/models/collection.rs`

| 模型 | 字段 | 校验规则 | 说明 |
|------|------|---------|------|
| `CollectionCreateParma` | `name` | `length(min=1, max=128)` | 相册名长度 |
| | `description` | `length(max=512)` | 描述长度（可选） |
| `CollectionUpdateParam` | `name` | `length(min=1, max=128)` | 相册名长度（可选） |
| | `description` | `length(max=512)` | 描述长度（可选） |
| `CollectionPhotoAddBatchParam` | `photo_ids` | `length(min=1, max=128)` | 批量添加数量限制 |
| `CollectionPhotoRemoveBatchParam` | `photo_ids` | `length(min=1, max=128)` | 批量移除数量限制 |
| `CollectionPhotoCursorPageQuery` | `size` | `range(min=1, max=1024)` | 分页大小 |

#### 2.2 Comment 相关模型

**文件**: `domains/photo/src/models/comment.rs`

| 模型 | 字段 | 校验规则 | 说明 |
|------|------|---------|------|
| `CommentPublishParam` | `content` | `length(min=1, max=1024)` | 评论内容长度 |
| `CommentCursorPageQuery` | `size` | `range(min=1, max=1024)` | 分页大小 |

#### 2.3 Photo 相关模型

**文件**: `domains/photo/src/models/photo.rs`

| 模型 | 字段 | 校验规则 | 说明 |
|------|------|---------|------|
| `Md5sExistParam` | `md5s` | `length(min=1, max=128)` | 批量查询数量限制 |
| `DeletePhotoParam` | `photo_ids` | `length(min=1, max=128)` | 批量删除数量限制 |
| `PhotoCursorQuery` | `size` | `range(min=1, max=1024)` | 分页大小 |

### 3. 控制器改动

#### 3.1 PhotoController

**文件**: `domains/photo/src/controllers/photo_controller.rs`

```rust
// 改动前
Json(data): Json<Md5sExistParam>
Json(data): Json<DeletePhotoParam>
Query(query): Query<PhotoCursorQuery>

// 改动后
ValidatedJson(data): ValidatedJson<Md5sExistParam>
ValidatedJson(data): ValidatedJson<DeletePhotoParam>
ValidatedQuery(query): ValidatedQuery<PhotoCursorQuery>
```

#### 3.2 CollectionController

**文件**: `domains/photo/src/controllers/collection_controller.rs`

```rust
// 改动前
Json(data): Json<CollectionCreateParma>
Json(param): Json<CollectionUpdateParam>

// 改动后
ValidatedJson(data): ValidatedJson<CollectionCreateParma>
ValidatedJson(param): ValidatedJson<CollectionUpdateParam>
```

#### 3.3 CollectionPhotoController

**文件**: `domains/photo/src/controllers/collection_photo_controller.rs`

```rust
// 改动前
Json(data): Json<CollectionPhotoAddBatchParam>
Json(data): Json<CollectionPhotoRemoveBatchParam>
Query(query): Query<CollectionPhotoCursorPageQuery>

// 改动后
ValidatedJson(data): ValidatedJson<CollectionPhotoAddBatchParam>
ValidatedJson(data): ValidatedJson<CollectionPhotoRemoveBatchParam>
ValidatedQuery(query): ValidatedQuery<CollectionPhotoCursorPageQuery>
```

#### 3.4 CommentController

**文件**: `domains/photo/src/controllers/comment_controller.rs`

```rust
// 改动前
Json(param): Json<CommentPublishParam>
Query(param): Query<CommentCursorPageQuery>

// 改动后
ValidatedJson(param): ValidatedJson<CommentPublishParam>
ValidatedQuery(param): ValidatedQuery<CommentCursorPageQuery>
```

### 4. 错误处理

校验失败时返回统一格式的 400 错误：

```json
{
    "code": 400,
    "msg": "name: 相册名长度在 1 到 128 个字符; description: 描述长度不能超过 512 个字符",
    "data": null
}
```

错误信息格式：`字段名: 错误描述`，多个错误用 `; ` 分隔。

## 实施顺序

1. **Phase 1**: 新增 `ValidatedQuery` 提取器
   - 创建 `common/src/extractors/validated_query.rs`
   - 在 `common/src/extractors/mod.rs` 中导出

2. **Phase 2**: Photo 模块模型添加校验
   - `domains/photo/src/models/collection.rs` - 添加 Validate derive 和校验规则
   - `domains/photo/src/models/comment.rs` - 添加 Validate derive 和校验规则
   - `domains/photo/src/models/photo.rs` - 添加 Validate derive 和校验规则

3. **Phase 3**: 控制器替换提取器
   - `domains/photo/src/controllers/photo_controller.rs`
   - `domains/photo/src/controllers/collection_controller.rs`
   - `domains/photo/src/controllers/collection_photo_controller.rs`
   - `domains/photo/src/controllers/comment_controller.rs`

4. **Phase 4**: 测试
   - 为每个新增校验的模型编写单元测试
   - 运行现有集成测试确保无破坏

## 测试策略

### 单元测试

在各模型文件中添加 `#[cfg(test)]` 模块，测试：
- 正向用例：符合校验规则的数据
- 反向用例：违反校验规则的数据
- 边界值：最小长度、最大长度、超出范围

示例：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_collection_create_param_valid() {
        let param = CollectionCreateParma {
            name: "My Album".to_string(),
            description: Some("A test album".to_string()),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_create_param_name_too_long() {
        let param = CollectionCreateParma {
            name: "a".repeat(129),
            description: None,
        };
        assert!(param.validate().is_err());
    }
}
```

### 集成测试

- 现有集成测试应继续通过
- 可选：新增参数校验失败的集成测试用例

## 依赖关系

- `validator = "0.20.0"`（已有）
- `common` 模块的 `validators` feature（已有）

## 影响范围

- `common` 模块：新增 1 个文件，修改 1 个文件
- `domains/photo` 模块：修改 7 个文件
- 不影响 `auth` 和 `user` 模块（已使用校验）
- 不影响数据库 schema 和业务逻辑
