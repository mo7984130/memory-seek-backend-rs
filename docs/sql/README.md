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
`photo_photo.comment_count`、`photo_collection.photo_count`、`photo_comment.like_count` 等是冗余字段，通过应用层维护，避免 COUNT 查询。

## 常见查询索引

| 查询场景 | 使用的索引 |
|---------|-----------|
| 用户的照片列表 | idx_photo_user_id |
| 按时间排序的照片 | idx_photo_created_at |
| 文件去重/秒传 | idx_photo_md5 |
| 文件唯一性 | uk_photo_file_id |
| 用户的收藏夹列表 | idx_collection_user_id |
| 收藏夹内容（分页） | idx_fp_collection_id_created_id |
| 照片所属收藏夹 | idx_fp_photo_id |
| 照片评论（时间排序） | idx_comment_photo_time |
| 照片高赞评论 | idx_comment_photo_likes |
| 用户的评论列表 | idx_comment_user_id |
| 收藏夹-照片唯一性 | uk_collection_photo |
| 评论点赞唯一性 | uk_comment_user_like |
