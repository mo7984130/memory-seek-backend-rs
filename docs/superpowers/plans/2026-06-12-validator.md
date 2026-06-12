# 控制器参数校验实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 Photo 模块所有控制器添加统一的参数校验，使用 validator crate 的 Validate derive 宏。

**Architecture:** 新增 ValidatedQuery 提取器（仿照 ValidatedJson），在模型层定义校验规则，控制器替换为 ValidatedJson/ValidatedQuery 提取器。

**Tech Stack:** Rust, Axum, validator 0.20.0, serde

---

## 文件结构

| 操作 | 文件路径 | 职责 |
|------|---------|------|
| Create | `common/src/extractors/validated_query.rs` | Query 参数校验提取器 |
| Modify | `common/src/extractors/mod.rs` | 导出 ValidatedQuery |
| Modify | `domains/photo/src/models/collection.rs` | Collection 相关模型添加 Validate |
| Modify | `domains/photo/src/models/comment.rs` | Comment 相关模型添加 Validate |
| Modify | `domains/photo/src/models/photo.rs` | Photo 相关模型添加 Validate |
| Modify | `domains/photo/src/controllers/photo_controller.rs` | 替换提取器 |
| Modify | `domains/photo/src/controllers/collection_controller.rs` | 替换提取器 |
| Modify | `domains/photo/src/controllers/collection_photo_controller.rs` | 替换提取器 |
| Modify | `domains/photo/src/controllers/comment_controller.rs` | 替换提取器 |

---

### Task 1: 创建 ValidatedQuery 提取器

**Files:**
- Create: `common/src/extractors/validated_query.rs`
- Modify: `common/src/extractors/mod.rs`

- [ ] **Step 1: 创建 validated_query.rs 文件**

```rust
// common/src/extractors/validated_query.rs
use axum::extract::{FromRequestParts, Query};
use serde::de::DeserializeOwned;
use std::ops::Deref;
use validator::Validate;

use crate::{
    error::AppError,
    ext::{ResultErrExt, log_warn},
};

/// 带自动验证的 Query 参数提取器
///
/// 组合了 Query 反序列化和 `validator` 校验，替代 axum 原生的 `Query` 提取器。
/// 当查询参数解析失败或校验不通过时，返回 400 状态码和错误详情。
pub struct ValidatedQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    /// 从请求 URI 解析查询参数并执行校验
    ///
    /// # 参数
    /// - `parts`: HTTP 请求的部分信息，从中提取查询字符串
    /// - `state`: axum 应用状态
    ///
    /// # 返回
    /// 返回校验通过的 `ValidatedQuery<T>` 包装值
    ///
    /// # 错误
    /// - `400 Bad Request`: 查询参数解析失败或字段校验不通过
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

/// 解引用到内部类型 `T`，方便直接调用 `T` 的方法
impl<T> Deref for ValidatedQuery<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
```

- [ ] **Step 2: 修改 mod.rs 导出 ValidatedQuery**

在 `common/src/extractors/mod.rs` 中添加：

```rust
#[cfg(feature = "validators")]
pub mod validated_query;

#[cfg(feature = "validators")]
pub use validated_query::ValidatedQuery;
```

完整文件内容：

```rust
/// 请求提取器模块
///
/// 提供自定义的 axum 请求提取器：
/// - `ValidatedJson`: 带 `validator` 校验的 JSON 请求体提取器
/// - `ValidatedQuery`: 带 `validator` 校验的查询参数提取器
/// - `ClientIp`: 客户端 IP 地址提取器，支持 `X-Real-IP` 头和 TCP 连接回退
#[cfg(feature = "validators")]
pub mod validated_json;
#[cfg(feature = "validators")]
pub mod validated_query;
pub mod client_ip;

#[cfg(feature = "validators")]
pub use validated_json::ValidatedJson;
#[cfg(feature = "validators")]
pub use validated_query::ValidatedQuery;
pub use client_ip::ClientIp;
```

- [ ] **Step 3: 验证编译通过**

Run: `cargo build --features "auth,user,photo"`
Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add common/src/extractors/validated_query.rs common/src/extractors/mod.rs
git commit -m "feat(common): 新增 ValidatedQuery 提取器"
```

---

### Task 2: Collection 模型添加校验

**Files:**
- Modify: `domains/photo/src/models/collection.rs`

- [ ] **Step 1: 添加 Validate derive 和校验规则**

在文件顶部添加 use 语句：

```rust
use validator::Validate;
```

修改 `CollectionCreateParma` 结构体：

```rust
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCreateParma {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: String,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
}
```

修改 `CollectionUpdateParam` 结构体：

```rust
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionUpdateParam {
    #[validate(length(min = 1, max = 128, message = "相册名长度在 1 到 128 个字符"))]
    pub name: Option<String>,
    #[validate(length(max = 512, message = "描述长度不能超过 512 个字符"))]
    pub description: Option<String>,
}
```

修改 `CollectionPhotoAddBatchParam` 结构体：

```rust
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoAddBatchParam {
    #[validate(length(min = 1, max = 128, message = "照片数量在 1 到 128 之间"))]
    pub photo_ids: Vec<PhotoId>,
}
```

修改 `CollectionPhotoRemoveBatchParam` 结构体：

```rust
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoRemoveBatchParam {
    #[validate(length(min = 1, max = 128, message = "照片数量在 1 到 128 之间"))]
    pub photo_ids: Vec<PhotoId>,
}
```

修改 `CollectionPhotoCursorPageQuery` 结构体：

```rust
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPhotoCursorPageQuery {
    pub cursor: Option<String>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    pub size: Option<u32>,
}
```

- [ ] **Step 2: 添加单元测试**

在文件末尾添加测试模块：

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
    fn test_collection_create_param_name_empty() {
        let param = CollectionCreateParma {
            name: "".to_string(),
            description: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_create_param_name_too_long() {
        let param = CollectionCreateParma {
            name: "a".repeat(129),
            description: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_create_param_description_too_long() {
        let param = CollectionCreateParma {
            name: "Album".to_string(),
            description: Some("a".repeat(513)),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_update_param_valid() {
        let param = CollectionUpdateParam {
            name: Some("New Name".to_string()),
            description: Some("New desc".to_string()),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_update_param_name_too_long() {
        let param = CollectionUpdateParam {
            name: Some("a".repeat(129)),
            description: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_photo_add_batch_param_valid() {
        let param = CollectionPhotoAddBatchParam {
            photo_ids: vec![PhotoId(1), PhotoId(2)],
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_photo_add_batch_param_empty() {
        let param = CollectionPhotoAddBatchParam {
            photo_ids: vec![],
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_collection_photo_cursor_page_query_valid() {
        let param = CollectionPhotoCursorPageQuery {
            cursor: None,
            size: Some(50),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_collection_photo_cursor_page_query_size_too_large() {
        let param = CollectionPhotoCursorPageQuery {
            cursor: None,
            size: Some(1025),
        };
        assert!(param.validate().is_err());
    }
}
```

- [ ] **Step 3: 运行单元测试**

Run: `cargo test --lib -p photo -- collection`
Expected: 所有测试通过

- [ ] **Step 4: 提交**

```bash
git add domains/photo/src/models/collection.rs
git commit -m "feat(photo): Collection 模型添加参数校验"
```

---

### Task 3: Comment 模型添加校验

**Files:**
- Modify: `domains/photo/src/models/comment.rs`

- [ ] **Step 1: 添加 Validate derive 和校验规则**

在文件顶部添加 use 语句：

```rust
use validator::Validate;
```

修改 `CommentPublishParam` 结构体：

```rust
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CommentPublishParam {
    #[validate(length(min = 1, max = 1024, message = "评论内容长度在 1 到 1024 个字符"))]
    pub content: String,
}
```

修改 `CommentCursorPageQuery` 结构体：

```rust
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CommentCursorPageQuery {
    pub cursor: Option<DateTimeUtc>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    pub size: Option<u64>,
}
```

- [ ] **Step 2: 添加单元测试**

在文件末尾添加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_comment_publish_param_valid() {
        let param = CommentPublishParam {
            content: "This is a comment".to_string(),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_publish_param_empty() {
        let param = CommentPublishParam {
            content: "".to_string(),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_comment_publish_param_too_long() {
        let param = CommentPublishParam {
            content: "a".repeat(1025),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_comment_cursor_page_query_valid() {
        let param = CommentCursorPageQuery {
            cursor: None,
            size: Some(50),
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_comment_cursor_page_query_size_too_large() {
        let param = CommentCursorPageQuery {
            cursor: None,
            size: Some(1025),
        };
        assert!(param.validate().is_err());
    }
}
```

- [ ] **Step 3: 运行单元测试**

Run: `cargo test --lib -p photo -- comment`
Expected: 所有测试通过

- [ ] **Step 4: 提交**

```bash
git add domains/photo/src/models/comment.rs
git commit -m "feat(photo): Comment 模型添加参数校验"
```

---

### Task 4: Photo 模型添加校验

**Files:**
- Modify: `domains/photo/src/models/photo.rs`

- [ ] **Step 1: 添加 Validate derive 和校验规则**

在文件顶部添加 use 语句：

```rust
use validator::Validate;
```

修改 `PhotoCursorQuery` 结构体：

```rust
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase", default)]
pub struct PhotoCursorQuery {
    pub cursor: Option<String>,
    #[validate(range(min = 1, max = 1024, message = "分页大小在 1 到 1024 之间"))]
    pub size: u64,
    pub direction: PageDirection,
    pub default_collection_id: Option<String>,
}
```

修改 `Md5sExistParam` 结构体：

```rust
#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Md5sExistParam {
    #[validate(length(min = 1, max = 128, message = "MD5 数量在 1 到 128 之间"))]
    pub md5s: Vec<String>,
}
```

修改 `DeletePhotoParam` 结构体：

```rust
#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct DeletePhotoParam {
    #[validate(length(min = 1, max = 128, message = "照片数量在 1 到 128 之间"))]
    pub photo_ids: Vec<PhotoId>,
}
```

- [ ] **Step 2: 添加单元测试**

在文件末尾添加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_photo_cursor_query_valid() {
        let param = PhotoCursorQuery {
            cursor: None,
            size: 50,
            direction: PageDirection::Next,
            default_collection_id: None,
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_photo_cursor_query_size_zero() {
        let param = PhotoCursorQuery {
            cursor: None,
            size: 0,
            direction: PageDirection::Next,
            default_collection_id: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_photo_cursor_query_size_too_large() {
        let param = PhotoCursorQuery {
            cursor: None,
            size: 1025,
            direction: PageDirection::Next,
            default_collection_id: None,
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_md5s_exist_param_valid() {
        let param = Md5sExistParam {
            md5s: vec!["abc123".to_string(), "def456".to_string()],
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_md5s_exist_param_empty() {
        let param = Md5sExistParam {
            md5s: vec![],
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_md5s_exist_param_too_many() {
        let param = Md5sExistParam {
            md5s: (0..129).map(|i| format!("md5_{}", i)).collect(),
        };
        assert!(param.validate().is_err());
    }

    #[test]
    fn test_delete_photo_param_valid() {
        let param = DeletePhotoParam {
            photo_ids: vec![PhotoId(1), PhotoId(2)],
        };
        assert!(param.validate().is_ok());
    }

    #[test]
    fn test_delete_photo_param_empty() {
        let param = DeletePhotoParam {
            photo_ids: vec![],
        };
        assert!(param.validate().is_err());
    }
}
```

- [ ] **Step 3: 运行单元测试**

Run: `cargo test --lib -p photo -- photo`
Expected: 所有测试通过

- [ ] **Step 4: 提交**

```bash
git add domains/photo/src/models/photo.rs
git commit -m "feat(photo): Photo 模型添加参数校验"
```

---

### Task 5: PhotoController 替换提取器

**Files:**
- Modify: `domains/photo/src/controllers/photo_controller.rs`

- [ ] **Step 1: 添加 ValidatedJson 和 ValidatedQuery 导入**

在文件顶部的 use 语句中添加：

```rust
use common::extractors::{ValidatedJson, ValidatedQuery};
```

移除不再需要的 `Json` 导入（如果存在）。

- [ ] **Step 2: 修改 upload 函数签名**

不需要修改 upload 函数，因为它使用 Multipart 而非 Json。

- [ ] **Step 3: 修改 get_photos_cursor 函数签名**

```rust
// 改动前
Query(query): Query<PhotoCursorQuery>,

// 改动后
ValidatedQuery(query): ValidatedQuery<PhotoCursorQuery>,
```

- [ ] **Step 4: 修改 md5s_exist 函数签名**

```rust
// 改动前
Json(data): Json<Md5sExistParam>,

// 改动后
ValidatedJson(data): ValidatedJson<Md5sExistParam>,
```

- [ ] **Step 5: 修改 delete_photos 函数签名**

```rust
// 改动前
Json(data): Json<DeletePhotoParam>,

// 改动后
ValidatedJson(data): ValidatedJson<DeletePhotoParam>,
```

- [ ] **Step 6: 验证编译通过**

Run: `cargo build --features "auth,photo"`
Expected: 编译成功

- [ ] **Step 7: 提交**

```bash
git add domains/photo/src/controllers/photo_controller.rs
git commit -m "refactor(photo): PhotoController 替换为 ValidatedJson/ValidatedQuery"
```

---

### Task 6: CollectionController 替换提取器

**Files:**
- Modify: `domains/photo/src/controllers/collection_controller.rs`

- [ ] **Step 1: 添加 ValidatedJson 导入**

在文件顶部的 use 语句中添加：

```rust
use common::extractors::ValidatedJson;
```

移除 `Json` 导入。

- [ ] **Step 2: 修改 create 函数签名**

```rust
// 改动前
Json(data): Json<CollectionCreateParma>,

// 改动后
ValidatedJson(data): ValidatedJson<CollectionCreateParma>,
```

- [ ] **Step 3: 修改 update_info 函数签名**

```rust
// 改动前
Json(param): Json<CollectionUpdateParam>,

// 改动后
ValidatedJson(param): ValidatedJson<CollectionUpdateParam>,
```

- [ ] **Step 4: 验证编译通过**

Run: `cargo build --features "auth,photo"`
Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add domains/photo/src/controllers/collection_controller.rs
git commit -m "refactor(photo): CollectionController 替换为 ValidatedJson"
```

---

### Task 7: CollectionPhotoController 替换提取器

**Files:**
- Modify: `domains/photo/src/controllers/collection_photo_controller.rs`

- [ ] **Step 1: 添加 ValidatedJson 和 ValidatedQuery 导入**

在文件顶部的 use 语句中添加：

```rust
use common::extractors::{ValidatedJson, ValidatedQuery};
```

移除 `Json` 和 `Query` 导入。

- [ ] **Step 2: 修改 add_photos 函数签名**

```rust
// 改动前
Json(data): Json<CollectionPhotoAddBatchParam>,

// 改动后
ValidatedJson(data): ValidatedJson<CollectionPhotoAddBatchParam>,
```

- [ ] **Step 3: 修改 remove_photos 函数签名**

```rust
// 改动前
Json(data): Json<CollectionPhotoRemoveBatchParam>,

// 改动后
ValidatedJson(data): ValidatedJson<CollectionPhotoRemoveBatchParam>,
```

- [ ] **Step 4: 修改 get_cursor_page 函数签名**

```rust
// 改动前
Query(query): Query<CollectionPhotoCursorPageQuery>,

// 改动后
ValidatedQuery(query): ValidatedQuery<CollectionPhotoCursorPageQuery>,
```

- [ ] **Step 5: 验证编译通过**

Run: `cargo build --features "auth,photo"`
Expected: 编译成功

- [ ] **Step 6: 提交**

```bash
git add domains/photo/src/controllers/collection_photo_controller.rs
git commit -m "refactor(photo): CollectionPhotoController 替换为 ValidatedJson/ValidatedQuery"
```

---

### Task 8: CommentController 替换提取器

**Files:**
- Modify: `domains/photo/src/controllers/comment_controller.rs`

- [ ] **Step 1: 添加 ValidatedJson 和 ValidatedQuery 导入**

在文件顶部的 use 语句中添加：

```rust
use common::extractors::{ValidatedJson, ValidatedQuery};
```

移除 `Json` 和 `Query` 导入。

- [ ] **Step 2: 修改 publish 函数签名**

```rust
// 改动前
Json(param): Json<CommentPublishParam>,

// 改动后
ValidatedJson(param): ValidatedJson<CommentPublishParam>,
```

- [ ] **Step 3: 修改 get_cursor_page 函数签名**

```rust
// 改动前
Query(param): Query<CommentCursorPageQuery>,

// 改动后
ValidatedQuery(param): ValidatedQuery<CommentCursorPageQuery>,
```

- [ ] **Step 4: 验证编译通过**

Run: `cargo build --features "auth,photo"`
Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add domains/photo/src/controllers/comment_controller.rs
git commit -m "refactor(photo): CommentController 替换为 ValidatedJson/ValidatedQuery"
```

---

### Task 9: 运行完整测试套件

**Files:**
- None (验证任务)

- [ ] **Step 1: 运行所有单元测试**

Run: `cargo test --lib`
Expected: 所有测试通过

- [ ] **Step 2: 运行 Photo 模块单元测试**

Run: `cargo test --lib -p photo`
Expected: 所有测试通过（包括新增的校验测试）

- [ ] **Step 3: 启动测试服务**

Run: `podman compose -f tests/load/docker-compose.yml up -d postgres redis minio`
Expected: 服务启动成功

- [ ] **Step 4: 运行集成测试**

Run: `cargo test --test integration --features "auth,user,photo" -- --test-threads=1`
Expected: 所有集成测试通过

- [ ] **Step 5: 提交最终版本**

```bash
git add -A
git commit -m "feat: 完成 Photo 模块参数校验"
```

---

## 验证清单

- [ ] ValidatedQuery 提取器创建并导出
- [ ] Collection 模型添加 Validate derive 和校验规则
- [ ] Comment 模型添加 Validate derive 和校验规则
- [ ] Photo 模型添加 Validate derive 和校验规则
- [ ] PhotoController 替换提取器
- [ ] CollectionController 替换提取器
- [ ] CollectionPhotoController 替换提取器
- [ ] CommentController 替换提取器
- [ ] 所有单元测试通过
- [ ] 所有集成测试通过
