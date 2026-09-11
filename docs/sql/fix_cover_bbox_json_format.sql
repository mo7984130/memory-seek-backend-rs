-- ============================================================
-- 数据修复: photo_person.cover_bbox 对象格式 → 数组格式
--
-- 背景
--   insight-face-rs 的 BoundingBox 以 JSON 数组 [x1, y1, x2, y2] 序列化
--   (#[serde(from = "[f32; 4]", into = "[f32; 4]")])。
--   但 tests/load/seed/seed.sql 曾把 cover_bbox 插入为 JSON 对象
--   {"x1":..,"y1":..,"x2":..,"y2":..}, 导致 sea-orm 解码时抛错:
--     ColumnDecode: invalid type: map, expected an array of length 4
--   (photo_face.bbox / landmarks 一直是数组格式, 不受影响。)
--
-- 本脚本把存量对象格式的 cover_bbox 原地转换为数组格式。
-- 幂等: 已是数组的行不满足 WHERE 条件, 可重复执行。
-- ============================================================

BEGIN;

UPDATE photo_person
SET cover_bbox = json_build_array(
    (cover_bbox->>'x1')::float8,
    (cover_bbox->>'y1')::float8,
    (cover_bbox->>'x2')::float8,
    (cover_bbox->>'y2')::float8
)
WHERE json_typeof(cover_bbox) = 'object';

COMMIT;
