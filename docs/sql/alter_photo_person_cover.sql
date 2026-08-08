-- ============================================================
-- photo_person 封面冗余字段迁移(消除 GET /photo/person/ N+1 查询)
-- ============================================================
-- 用途: 分页读取 person 时, cover_token 由 photo_person 冗余字段直接内存组装,
--       不再逐条查询 photo_face / photo_photo(查询数 1+2N → 1)。
-- 适用: 已有存量 photo_person 数据的数据库; 全新库直接用 init.sql, 无需本脚本。
-- 前置: photo_face.bbox 需为归一化坐标(insight-face-rs 2.x 起为相对坐标 [0,1])。
--       若此前已跑过 face-engine 全量重算(清空 face/person 后重新检测聚类),
--       新 person 行已由新代码写入冗余字段, 本脚本仅补历史行, 可安全重复执行。
-- ============================================================

-- 1. 加列(先可空, 回填完成后收紧)
ALTER TABLE photo_person
    ADD COLUMN IF NOT EXISTS cover_photo_id BIGINT,
    ADD COLUMN IF NOT EXISTS cover_file_id  VARCHAR(255),
    ADD COLUMN IF NOT EXISTS cover_bbox     JSONB;

-- 2. 回填: 经 cover_face_id → photo_face → photo_photo 补齐
--    注意: photo_face.bbox 为 JSON 数组 [x1,y1,x2,y2](insight-face-rs 的
--    BoundingBox 序列化格式), 需转换为项目 FaceBBox 的对象格式
--    {"x1":..,"y1":..,"x2":..,"y2":..}, 否则读取时反序列化失败。
UPDATE photo_person p
SET cover_photo_id = f.photo_id,
    cover_file_id  = ph.file_id,
    cover_bbox     = jsonb_build_object(
                        'x1', f.bbox->0,
                        'y1', f.bbox->1,
                        'x2', f.bbox->2,
                        'y2', f.bbox->3
                    )
FROM photo_face f
JOIN photo_photo ph ON ph.id = f.photo_id
WHERE f.id = p.cover_face_id
  AND (p.cover_file_id IS NULL OR p.cover_bbox IS NULL);

-- 3. 校验: 期望返回 0 行(有返回说明 cover face/photo 缺失, 需人工处理后再收紧)
SELECT id, name, cover_face_id
FROM photo_person
WHERE cover_photo_id IS NULL OR cover_file_id IS NULL OR cover_bbox IS NULL;

-- 4. 收紧非空(确认第 3 步为 0 行后执行)
ALTER TABLE photo_person
    ALTER COLUMN cover_photo_id SET NOT NULL,
    ALTER COLUMN cover_file_id  SET NOT NULL,
    ALTER COLUMN cover_bbox     SET NOT NULL;

-- 5. 列注释
COMMENT ON COLUMN photo_person.cover_photo_id IS '封面人脸所属照片ID(冗余自 photo_face.photo_id, 消除封面查询 N+1)';
COMMENT ON COLUMN photo_person.cover_file_id IS '封面照片 file_id(冗余自 photo_photo.file_id, 消除封面查询 N+1)';
COMMENT ON COLUMN photo_person.cover_bbox IS '封面人脸归一化 bbox(冗余自 photo_face.bbox, 消除封面查询 N+1)';
