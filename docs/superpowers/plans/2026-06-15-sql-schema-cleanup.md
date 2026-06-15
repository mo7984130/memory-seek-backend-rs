# SQL Schema 清理与优化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 清理冗余 SQL 文件，删除弃用的人脸/向量功能，修复 schema 不一致问题，添加缺失索引

**Architecture:** 删除 `auth/` 和 `photo/` 文件夹中的重复文件，修改 `init.sql` 移除弃用表并修复 schema 问题，新建 `README.md` 提供架构文档

**Tech Stack:** PostgreSQL 16, SQL

---

### Task 1: 删除冗余 SQL 文件

**Files:**
- Delete: `docs/sql/auth/auth.sql`
- Delete: `docs/sql/auth/test_data.sql`
- Delete: `docs/sql/photo/collection.sql`
- Delete: `docs/sql/photo/comment.sql`
- Delete: `docs/sql/photo/face.sql`
- Delete: `docs/sql/photo/photo.sql`
- Delete: `docs/sql/auth/` (空目录)
- Delete: `docs/sql/photo/` (空目录)

- [ ] **Step 1: 删除 auth 目录下的文件**

```bash
rm docs/sql/auth/auth.sql docs/sql/auth/test_data.sql
rmdir docs/sql/auth
```

- [ ] **Step 2: 删除 photo 目录下的文件**

```bash
rm docs/sql/photo/collection.sql docs/sql/photo/comment.sql docs/sql/photo/face.sql docs/sql/photo/photo.sql
rmdir docs/sql/photo
```

- [ ] **Step 3: 验证目录结构**

```bash
ls -la docs/sql/
```

Expected: 只有 `init.sql` 文件

- [ ] **Step 4: Commit**

```bash
git add -A docs/sql/
git commit -m "chore: 删除冗余的 auth 和 photo SQL 文件"
```

---

### Task 2: 修改 init.sql - 删除人脸/向量相关内容

**Files:**
- Modify: `docs/sql/init.sql`

- [ ] **Step 1: 删除 vector 扩展**

删除以下行：
```sql
-- 开启向量拓展
CREATE EXTENSION IF NOT EXISTS vector;
```

- [ ] **Step 2: 删除 photo_face_person 表及其索引**

删除以下内容：
```sql
-- 人物信息表
CREATE TABLE IF NOT EXISTS photo_face_person(
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    name_initials VARCHAR(50),
    max_score_feature_id BIGINT NOT NULL,
    max_score FLOAT4 DEFAULT 0.0 NOT NULL,
    total_photo_count BIGINT DEFAULT 0 NOT NULL, -- 总图片数
    centroid_embedding vector(512) default '[0.0]' NOT NULL, -- 中心特征向量
    total_weight_count FLOAT4 DEFAULT 0.0 NOT NULL, -- 总权重
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT uk_person_name UNIQUE (name)
);
COMMENT ON TABLE photo_face_person IS '人物信息表';
COMMENT ON COLUMN photo_face_person.id IS '人物ID';
COMMENT ON COLUMN photo_face_person.name IS '人物名称';
COMMENT ON COLUMN photo_face_person.name_initials IS '名字首字母（拼音）';
COMMENT ON COLUMN photo_face_person.max_score_feature_id IS '最大置信度特征ID';
COMMENT ON COLUMN photo_face_person.total_photo_count IS '总图片数';
COMMENT ON COLUMN photo_face_person.centroid_embedding IS '中心特征向量';
COMMENT ON COLUMN photo_face_person.total_weight_count IS '总权重';
COMMENT ON COLUMN photo_face_person.created_at IS '创建时间';
COMMENT ON COLUMN photo_face_person.updated_at IS '更新时间';

CREATE INDEX IF NOT EXISTS idx_person_name_initials ON photo_face_person (name_initials);
```

- [ ] **Step 3: 删除 photo_face_feature 表及其索引**

删除以下内容：
```sql
-- 人脸特征表
CREATE TABLE IF NOT EXISTS photo_face_feature(
    id BIGSERIAL PRIMARY KEY,
    photo_id BIGINT NOT NULL,
    person_id BIGINT NULL,
    embedding vector(512) NOT NULL,
    bbox JSONB NOT NULL,
    score FLOAT4 NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);
COMMENT ON TABLE photo_face_feature IS '人脸特征表';
COMMENT ON COLUMN photo_face_feature.id IS '特征ID';
COMMENT ON COLUMN photo_face_feature.photo_id IS '图片ID';
COMMENT ON COLUMN photo_face_feature.person_id IS '人物ID';
COMMENT ON COLUMN photo_face_feature.embedding IS '人脸特征向量';
COMMENT ON COLUMN photo_face_feature.bbox IS '人脸边界框';
COMMENT ON COLUMN photo_face_feature.score IS '置信度';
COMMENT ON COLUMN photo_face_feature.created_at IS '创建时间';
COMMENT ON COLUMN photo_face_feature.updated_at IS '更新时间';
-- 该索引先按人物分组，再在组内按分数倒序排列
CREATE INDEX IF NOT EXISTS idx_feature_person_score
    ON photo_face_feature (person_id, score DESC);

-- 创建向量索引
CREATE INDEX idx_photo_face_feature_embedding ON photo_face_feature USING hnsw (embedding vector_cosine_ops);
-- 创建人物索引
CREATE INDEX idx_photo_face_feature_person_id ON photo_face_feature USING btree (person_id);
```

- [ ] **Step 4: 验证修改后的 init.sql**

```bash
grep -n "vector\|face" docs/sql/init.sql
```

Expected: 无输出（所有 vector 和 face 相关内容已删除）

- [ ] **Step 5: Commit**

```bash
git add docs/sql/init.sql
git commit -m "refactor: 从 init.sql 删除人脸/向量相关表"
```

---

### Task 3: 修改 init.sql - 修复 photo_collection 表

**Files:**
- Modify: `docs/sql/init.sql`

- [ ] **Step 1: 修改 photo_collection 表的 created_at 和 updated_at**

找到：
```sql
CREATE TABLE IF NOT EXISTS photo_collection (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    photo_count BIGINT DEFAULT 0 NOT NULL,
    cover_file_id VARCHAR,
    is_favorite BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
```

替换为：
```sql
CREATE TABLE IF NOT EXISTS photo_collection (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    photo_count BIGINT DEFAULT 0 NOT NULL,
    cover_file_id VARCHAR,
    is_favorite BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 2: 验证修改**

```bash
grep -A 2 "created_at TIMESTAMPTZ NOT NULL" docs/sql/init.sql | grep -v "photo_photo\|photo_comment\|photo_comment_like"
```

Expected: 显示 `DEFAULT NOW()` 在 photo_collection 的字段中

- [ ] **Step 3: Commit**

```bash
git add docs/sql/init.sql
git commit -m "fix: 给 photo_collection 的时间字段添加 DEFAULT NOW()"
```

---

### Task 4: 修改 init.sql - 添加缺失索引

**Files:**
- Modify: `docs/sql/init.sql`

- [ ] **Step 1: 在 photo_photo 表定义后添加 user_id 索引**

在 `CREATE INDEX idx_photo_md5 ON photo_photo (md5);` 后添加：
```sql
CREATE INDEX idx_photo_user_id ON photo_photo (user_id);
```

- [ ] **Step 2: 在 photo_comment 表定义后添加 user_id 索引**

在 `CREATE INDEX idx_comment_photo_likes ON photo_comment (photo_id, like_count DESC);` 后添加：
```sql
CREATE INDEX idx_comment_user_id ON photo_comment (user_id);
```

- [ ] **Step 3: 在 photo_collection_photo 表定义后添加 photo_id 索引**

在 `CREATE INDEX idx_fp_collection_id_created_id ON photo_collection_photo(collection_id, created_at DESC, id DESC);` 后添加：
```sql
CREATE INDEX idx_fp_photo_id ON photo_collection_photo (photo_id);
```

- [ ] **Step 4: 验证新增索引**

```bash
grep "CREATE INDEX" docs/sql/init.sql
```

Expected: 包含 `idx_photo_user_id`, `idx_comment_user_id`, `idx_fp_photo_id`

- [ ] **Step 5: Commit**

```bash
git add docs/sql/init.sql
git commit -m "feat: 添加缺失的查询索引"
```

---

### Task 5: 创建 README.md

**Files:**
- Create: `docs/sql/README.md`

- [ ] **Step 1: 创建 README.md**

```markdown
# 数据库 Schema 说明

## 概述
Memory Seek 数据库使用 PostgreSQL 16。

## 表结构

### Auth 模块
- `auth_user` - 用户表（用户名、邮箱、密码、token）

### Photo 模块
- `photo_photo` - 照片表
- `photo_timeline_stat` - 时间线统计
- `photo_collection` - 收藏夹
- `photo_collection_photo` - 收藏关系（多对多）
- `photo_comment` - 评论
- `photo_comment_like` - 评论点赞

## 设计决策

### 无外键约束
所有表不使用 FOREIGN KEY，关联关系由应用层保证。
- **优点：** 写入性能更好，删除/更新更灵活
- **缺点：** 需要代码层面保证数据完整性

### 冗余计数字段
`photo_photo.comment_count`、`photo_collection.photo_count` 等是冗余字段，通过应用层维护，避免 COUNT 查询。

## 常见查询索引

| 查询场景 | 使用的索引 |
|---------|-----------|
| 用户的照片列表 | idx_photo_user_id |
| 收藏夹内容（分页） | idx_fp_collection_id_created_id |
| 照片评论（时间排序） | idx_comment_photo_time |
```

- [ ] **Step 2: 验证文件创建**

```bash
cat docs/sql/README.md
```

Expected: 显示完整内容

- [ ] **Step 3: Commit**

```bash
git add docs/sql/README.md
git commit -m "docs: 添加数据库 Schema 说明文档"
```

---

### Task 6: 最终验证

**Files:**
- Read: `docs/sql/init.sql`
- Read: `docs/sql/README.md`

- [ ] **Step 1: 验证目录结构**

```bash
ls -la docs/sql/
```

Expected: 只有 `init.sql` 和 `README.md`

- [ ] **Step 2: 验证 init.sql 不包含弃用内容**

```bash
grep -i "vector\|face_person\|face_feature" docs/sql/init.sql
```

Expected: 无输出

- [ ] **Step 3: 验证 init.sql 包含所有必要的表**

```bash
grep "CREATE TABLE" docs/sql/init.sql
```

Expected:
```
CREATE TABLE IF NOT EXISTS auth_user
CREATE TABLE IF NOT EXISTS photo_photo
CREATE TABLE IF NOT EXISTS photo_timeline_stat
CREATE TABLE IF NOT EXISTS photo_collection
CREATE TABLE IF NOT EXISTS photo_collection_photo
CREATE TABLE IF NOT EXISTS photo_comment
CREATE TABLE IF NOT EXISTS photo_comment_like
```

- [ ] **Step 4: 验证新增索引存在**

```bash
grep "CREATE INDEX" docs/sql/init.sql
```

Expected: 包含 `idx_photo_user_id`, `idx_comment_user_id`, `idx_fp_photo_id`

- [ ] **Step 5: 验证 photo_collection 有 DEFAULT NOW()**

```bash
grep -A 15 "CREATE TABLE IF NOT EXISTS photo_collection" docs/sql/init.sql | grep "created_at"
```

Expected: `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),`
