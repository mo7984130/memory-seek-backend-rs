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
