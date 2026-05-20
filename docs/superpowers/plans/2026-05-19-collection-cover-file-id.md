# Collection Cover File ID 重构实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将收藏夹封面字段从 `cover_image_id`（引用 photo.id）改为 `cover_file_id`（直接存储 file_id 字符串），消除获取收藏夹列表时的二次查询。

**Architecture:** 修改 entity 字段类型，适配 mapper 层参数，简化 service 层查询逻辑。数据库迁移由用户手动执行。

**Tech Stack:** Rust, Sea-ORM, axum

---

## 涉及文件

| 文件 | 操作 | 职责 |
|------|------|------|
| `entities/src/photo_entities/collection.rs:13` | Modify | 字段 `cover_image_id` → `cover_file_id` |
| `domains/photo/src/mappers/collection_mapper.rs:113,131-141,155-156` | Modify | insert/update 参数适配 |
| `domains/photo/src/services/collection_service.rs:58-92,175-177` | Modify | 简化 get_collection_list，修复 edit_collection |
| `docs/sql/photo/collection.sql:8,21` | Modify | 更新建表语句和注释 |

---

### Task 1: 修改 Entity 字段

**Files:**
- Modify: `entities/src/photo_entities/collection.rs:13`

- [ ] **Step 1: 修改 collection entity 字段**

将第 13 行的 `cover_image_id: Option<i64>` 改为 `cover_file_id: Option<String>`：

```rust
// entities/src/photo_entities/collection.rs 第 13 行
// 原: pub cover_image_id: Option<i64>,
pub cover_file_id: Option<String>,
```

- [ ] **Step 2: 验证编译**

```bash
cargo check --package entities 2>&1 | head -20
```

Expected: 仅有下游引用错误（mapper/service），entity 本身无错。

- [ ] **Step 3: Commit**

```bash
git add entities/src/photo_entities/collection.rs
git commit -m "refactor(entity): rename cover_image_id to cover_file_id"
```

---

### Task 2: 适配 Mapper 层

**Files:**
- Modify: `domains/photo/src/mappers/collection_mapper.rs:108-121,131-161`

- [ ] **Step 1: 修改 insert 方法**

第 113 行，`cover_image_id: Set(None)` → `cover_file_id: Set(None)`：

```rust
// collection_mapper.rs 第 108-121 行
let collection = collection::ActiveModel {
    user_id: Set(user_id),
    name: Set(name),
    description: Set(description),
    photo_count: Set(0),
    cover_file_id: Set(None),
    is_favorite: Set(is_favorite),
    created_at: Set(now.into()),
    updated_at: Set(now.into()),
    ..Default::default()
};
```

- [ ] **Step 2: 修改 update 方法签名和实现**

第 131-161 行，参数类型从 `Option<Option<i64>>` 改为 `Option<Option<String>>`：

```rust
// collection_mapper.rs 第 135-161 行
pub async fn update(
    db: &DatabaseConnection,
    id: i64,
    name: Option<String>,
    description: Option<String>,
    photo_count: Option<i64>,
    cover_file_id: Option<Option<String>>,
) -> Result<collection::Model, AppError> {
    let existing = Self::query_by_id(db, id).await?;
    let mut active: collection::ActiveModel = existing.into();

    if let Some(n) = name {
        active.name = Set(n);
    }
    if let Some(d) = description {
        active.description = Set(Some(d));
    }
    if let Some(c) = photo_count {
        active.photo_count = Set(c);
    }
    if let Some(c) = cover_file_id {
        active.cover_file_id = Set(c);
    }
    active.updated_at = Set(Utc::now().into());

    active.update(db).await.trace_internal_err("db_update_err","更新收藏夹失败")
}
```

- [ ] **Step 3: 验证编译**

```bash
cargo check --package photo 2>&1 | head -30
```

Expected: 仅有 service 层引用错误。

- [ ] **Step 4: Commit**

```bash
git add domains/photo/src/mappers/collection_mapper.rs
git commit -m "refactor(mapper): adapt collection mapper for cover_file_id"
```

---

### Task 3: 简化 Service 层

**Files:**
- Modify: `domains/photo/src/services/collection_service.rs`

- [ ] **Step 1: 简化 get_collection_list 方法**

移除第 58-79 行的注释代码，将第 81-112 行的 map 逻辑简化为直接使用 `cover_file_id`：

```rust
// collection_service.rs 第 43-113 行，替换为：
pub async fn get_collection_list(
    state: &PhotoState,
    user_id: i64,
) -> Result<Vec<CollectionVO>, AppError> {
    let collections = CollectionMapper::query_by_user_id(&state.db, user_id).await?;

    let collections = if collections.is_empty() {
        Self::create_favorite_collection(state, user_id).await?;
        CollectionMapper::query_by_user_id(&state.db, user_id).await?
    } else {
        collections
    };

    let result: Vec<CollectionVO> = collections
        .into_iter()
        .map(|c| {
            let cover_token = c.cover_file_id.as_ref().and_then(|fid| {
                let (thumbnail_token, _, _) =
                    crate::models::photo::PhotoVO::generate_tokens(fid, &state.token_cipher);
                thumbnail_token
            });

            CollectionVO {
                id: c.id.to_string(),
                name: c.name,
                description: c.description,
                photo_count: c.photo_count,
                cover_token,
                is_favorite: c.is_favorite,
                created_at: c.created_at.with_timezone(&Utc),
            }
        })
        .collect();

    Ok(result)
}
```

- [ ] **Step 2: 修复 edit_collection 方法**

第 179-188 行，当前 `cover_token` 写死 `None`，改为从 `cover_file_id` 生成：

```rust
// collection_service.rs 第 179-188 行，替换为：
let cover_token = collection.cover_file_id.as_ref().and_then(|fid| {
    let (thumbnail_token, _, _) =
        crate::models::photo::PhotoVO::generate_tokens(fid, &state.token_cipher);
    thumbnail_token
});

Ok(CollectionVO {
    id: collection.id.to_string(),
    name: collection.name,
    description: collection.description,
    photo_count: collection.photo_count,
    cover_token,
    is_favorite: collection.is_favorite,
    created_at: collection.created_at.with_timezone(&Utc),
})
```

- [ ] **Step 3: 验证编译**

```bash
cargo check --features "photo" 2>&1 | head -30
```

Expected: 编译通过，无错误。

- [ ] **Step 4: Commit**

```bash
git add domains/photo/src/services/collection_service.rs
git commit -m "refactor(service): simplify collection list with cover_file_id"
```

---

### Task 4: 更新 SQL 文档

**Files:**
- Modify: `docs/sql/photo/collection.sql:8,21`

- [ ] **Step 1: 更新建表语句**

第 8 行 `cover_image_id BIGINT,` → `cover_file_id VARCHAR,`

第 21 行注释更新：

```sql
cover_file_id VARCHAR,
```

```sql
COMMENT ON COLUMN photo_collection.cover_file_id IS '收藏夹封面图的文件ID';
```

- [ ] **Step 2: Commit**

```bash
git add docs/sql/photo/collection.sql
git commit -m "docs(sql): update collection schema for cover_file_id"
```

---

### Task 5: 全量验证

- [ ] **Step 1: 全量编译检查**

```bash
cargo build 2>&1 | tail -20
```

Expected: 编译通过。

- [ ] **Step 2: 运行测试**

```bash
cargo test 2>&1 | tail -20
```

Expected: 所有测试通过。

- [ ] **Step 3: 最终 Commit（如有修复）**

```bash
git add -A
git commit -m "fix: address review feedback for cover_file_id refactor"
```
