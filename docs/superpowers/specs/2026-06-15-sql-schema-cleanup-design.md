# SQL Schema 清理与优化设计

**日期：** 2026-06-15
**状态：** 已批准

## 背景

`docs/sql/` 目录下存在多个 SQL 文件，其中 `auth/` 和 `photo/` 文件夹中的内容与 `init.sql` 完全重复。同时，部分已弃用的功能（人脸向量）需要清理，schema 中存在一些不一致的地方需要修复。

## 目标

1. 消除文件冗余，只保留单一 `init.sql` 作为 schema 定义
2. 删除已弃用的人脸/向量相关表
3. 修复 schema 中的不一致问题
4. 添加缺失的索引优化查询性能
5. 提供 README.md 文档说明设计决策

## 设计决策

### 无外键约束
所有表不使用 FOREIGN KEY，关联关系由应用层保证。
- **优点：** 写入性能更好，删除/更新更灵活
- **缺点：** 需要代码层面保证数据完整性

### 删除人脸/向量功能
以下内容不再使用，将从 schema 中移除：
- `CREATE EXTENSION IF NOT EXISTS vector;`
- `photo_face_person` 表
- `photo_face_feature` 表及相关索引

## 实施内容

### 1. 文件删除

删除以下冗余文件：
```
docs/sql/auth/auth.sql
docs/sql/auth/test_data.sql
docs/sql/photo/collection.sql
docs/sql/photo/comment.sql
docs/sql/photo/face.sql
docs/sql/photo/photo.sql
```

删除后删除空目录：
```
docs/sql/auth/
docs/sql/photo/
```

### 2. Schema 修复（init.sql）

#### 2.1 添加默认值
```sql
-- photo_collection 表，原来 created_at/updated_at 没有 DEFAULT NOW()
created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
```

#### 2.2 添加缺失索引
```sql
CREATE INDEX idx_photo_user_id ON photo_photo (user_id);
CREATE INDEX idx_comment_user_id ON photo_comment (user_id);
CREATE INDEX idx_fp_photo_id ON photo_collection_photo (photo_id);
```

#### 2.3 删除内容
- `CREATE EXTENSION IF NOT EXISTS vector;`
- `photo_face_person` 表定义及索引
- `photo_face_feature` 表定义及索引

### 3. 新增 README.md

创建 `docs/sql/README.md`，包含：
- 表结构概述
- 设计决策说明（无外键、冗余计数字段）
- 常见查询索引对照表

## 最终文件结构

```
docs/sql/
├── init.sql      ← 唯一的 schema 定义文件
└── README.md     ← 架构说明文档
```

## 验证

1. 确认 `init.sql` 可以正常执行创建所有表
2. 确认删除的文件不再被其他地方引用
3. 确认 README.md 内容准确
