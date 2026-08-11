-- tests/load/seed/schema_align.sql
-- 补齐 docs/sql/init.sql 与当前代码不一致的列(幂等)。
-- init.sql 尚未维护的演进列在此处对齐, 保证压测环境与代码实体一致。
-- 注意: 这里仅做压测环境对齐, 生产 schema 治理不在压测体系范围内。

-- photo_collection: 代码 Collection 实体含 cover_photo_id, init.sql 缺失
ALTER TABLE photo_collection
    ADD COLUMN IF NOT EXISTS cover_photo_id BIGINT;

-- photo_face.score: 代码 Face 实体为 f32(FLOAT4), init.sql 定义为 DOUBLE PRECISION(FLOAT8),
-- SQLx 无法将 FLOAT8 解码到 f32, 压测环境对齐为 REAL
ALTER TABLE photo_face ALTER COLUMN score TYPE REAL;

-- photo_face: get_unassigned_face_photos 用 EXISTS 子查询过滤 person_id IS NULL,
-- 普通 person_id 索引对 IS NULL 不高效, 部分索引大幅缩小扫描范围(seed 未分配脸占多数)
CREATE INDEX IF NOT EXISTS idx_photo_face_unassigned
    ON photo_face(photo_id) WHERE person_id IS NULL;

-- photo_photo: init.sql 中 idx_photo_created_at 演进为 (created_at DESC, id DESC),
-- 供 keyset 分页排序直接走索引; 此处对齐压测环境。
-- 老库已有旧单列索引时 CREATE IF NOT EXISTS 会静默跳过, 故先 DROP 再重建, 保证复合索引生效。
DROP INDEX IF EXISTS idx_photo_created_at;
CREATE INDEX idx_photo_created_at
    ON photo_photo (created_at DESC, id DESC);
