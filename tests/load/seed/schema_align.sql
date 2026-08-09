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
