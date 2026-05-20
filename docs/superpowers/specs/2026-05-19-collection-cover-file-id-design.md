# Collection Cover: cover_image_id -> cover_file_id

## 目标

将收藏夹封面字段从 `cover_image_id`（引用 `photo.id`）改为 `cover_file_id`（直接存储 `file_id` 字符串），消除获取收藏夹列表时查 photo 表的二次查询。

## 动机

当前 `get_collection_list` 流程：
1. 查询用户的收藏夹列表
2. 收集所有 `cover_image_id`，批量查 photo 表获取 `file_id`
3. 用 `file_id` 生成加密 token

改为 `cover_file_id` 后，步骤 2 完全省略，直接从收藏夹记录中取 `file_id` 生成 token。

## 设计

### 数据模型变更

`collection::Model`：
- 移除：`cover_image_id: Option<i64>`
- 新增：`cover_file_id: Option<String>`

### 涉及文件

| 文件 | 改动 |
|------|------|
| `entities/src/photo_entities/collection.rs` | 字段重命名和类型变更 |
| `docs/sql/photo/collection.sql` | 更新建表语句和注释 |
| `domains/photo/src/mappers/collection_mapper.rs` | `insert()` 和 `update()` 参数适配 |
| `domains/photo/src/services/collection_service.rs` | 简化 `get_collection_list()`，移除注释掉的降级逻辑 |

### Service 层改动

`get_collection_list()` 简化为：
```rust
let cover_token = c.cover_file_id.as_ref().and_then(|fid| {
    let (thumbnail_token, _, _) =
        PhotoVO::generate_tokens(fid, &state.token_cipher);
    thumbnail_token
});
```

移除所有注释掉的 `no_cover_ids`、`latest_photo_map`、`all_photo_map` 相关代码。

`edit_collection()` 返回 VO 时，如果有 `cover_file_id` 也生成 cover_token（当前写死 `None`）。

### 降级策略

不保留「没有封面时自动取最新照片」的降级逻辑。没有设置封面的收藏夹直接返回 `cover_token: None`。

### 数据库迁移

```sql
ALTER TABLE photo_collection ADD COLUMN cover_file_id VARCHAR;
UPDATE photo_collection c SET cover_file_id = (
    SELECT p.file_id FROM photo_photo p WHERE p.id = c.cover_image_id
);
ALTER TABLE photo_collection DROP COLUMN cover_image_id;
COMMENT ON COLUMN photo_collection.cover_file_id IS '收藏夹封面图的文件ID';
```

最后记得提交git
