-- tests/load/seed/schema_align.sql
-- init.sql 是完整的建表来源。本文件仅保留用于压测查询的补充索引。

-- get_unassigned_face_photos 用 EXISTS 子查询过滤 person_id IS NULL,
-- 普通 person_id 索引对 IS NULL 不高效, 部分索引大幅缩小扫描范围(seed 未分配脸占多数)
CREATE INDEX IF NOT EXISTS idx_photo_face_unassigned
    ON photo_face(photo_id) WHERE person_id IS NULL;

-- photo_photo: init.sql 中 idx_photo_created_at 演进为 (created_at DESC, id DESC),
-- 供 keyset 分页排序直接走索引; 此处对齐压测环境。
-- 老库已有旧单列索引时 CREATE IF NOT EXISTS 会静默跳过, 故先 DROP 再重建, 保证复合索引生效。
DROP INDEX IF EXISTS idx_photo_created_at;
CREATE INDEX idx_photo_created_at
    ON photo_photo (created_at DESC, id DESC);
