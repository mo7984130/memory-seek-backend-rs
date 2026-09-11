-- ============================================================
-- 迁移: Model 无符号整数 → 有符号整数 (postgres 不支持无符号)
--
-- 背景
--   sea-orm 2.0 在 postgres 上对无符号整型支持不完整:
--   - u32 / u64 建表时都被渲染为 bigint (INT8)
--   - 解码时 u32 按 integer (INT4) 读取, u64 直接不支持
--   导致运行时报错: "u64 unsupported by sqlx-postgres" / ColumnDecode INT4 vs INT8。
--
--   代码侧已把所有 Model 改为有符号类型 (u64→i64, u32→i32),
--   本脚本把已存在的 u32 列 (bigint) 收敛为 integer, 与新的 Model 对齐。
--
-- 幂等性
--   列已是目标类型时 ALTER TYPE 为 no-op, 可重复执行。
--   全新环境(空库)由 schema-sync 按新 Model 建表, 无需执行本脚本。
-- ============================================================

BEGIN;

-- photo_photo.width / height: u32 → i32 (bigint → integer)
ALTER TABLE photo_photo
    ALTER COLUMN width  TYPE integer USING width::integer,
    ALTER COLUMN height TYPE integer USING height::integer;

-- photo_comment.like_count: u32 → i32 (bigint → integer)
ALTER TABLE photo_comment
    ALTER COLUMN like_count TYPE integer USING like_count::integer;

-- 以下列原为 u64 → bigint, 新 Model 为 i64 → bigint, 类型不变, 无需 ALTER:
--   photo_photo.size / like_count / comment_count
--   photo_collection.photo_count
--   photo_person.face_count
--   photo_timeline_stat.count

COMMIT;
