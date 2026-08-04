-- ============================================================
-- photo_person 增量维护字段迁移(人脸归属转移重构)
-- ============================================================
-- 用途: 为 `change_face_belonging` / `merge_person` 的增量维护提供 weight 列,
--       并同步把 centroid 语义从「无权简单平均(归一化)」改为
--       「score 加权向量和 Σ(score*embedding), 未归一化」。
-- 适用: 已有存量 photo_person 数据的数据库; 全新库直接用 init.sql, 无需本脚本。
-- 前置: pgvector >= 0.5(AVG(vector) 聚合与 vector*float 标量乘);
--       版本不足时先 `ALTER EXTENSION vector UPDATE`。
-- ============================================================

-- 1. 加列(有默认值, 存量行直接可用)
ALTER TABLE photo_person ADD COLUMN IF NOT EXISTS weight DOUBLE PRECISION NOT NULL DEFAULT 0;

-- 2. 回填 weight: 该人物所有人脸 score 之和
UPDATE photo_person p
SET weight = COALESCE(sub.w, 0)
FROM (SELECT person_id, SUM(score) AS w
      FROM photo_face WHERE person_id IS NOT NULL GROUP BY person_id) sub
WHERE sub.person_id = p.id;

-- 3. 回填 centroid: score 加权和(旧值为无权简单平均, 与增量公式不兼容, 必须回填)
--    AVG(embedding * score) * COUNT(*) = Σ(score * embedding)
UPDATE photo_person p
SET centroid = sub.wsum
FROM (SELECT person_id, AVG(embedding * score) * COUNT(*) AS wsum
      FROM photo_face WHERE person_id IS NOT NULL GROUP BY person_id) sub
WHERE sub.person_id = p.id;

-- 4. 校验一致性(期望 0 行; 浮点比较带容差)
SELECT p.id, p.face_count, p.weight
FROM photo_person p
LEFT JOIN (SELECT person_id, COUNT(*) AS c, SUM(score) AS w
           FROM photo_face WHERE person_id IS NOT NULL GROUP BY person_id) f
       ON f.person_id = p.id
WHERE f.c IS NULL
   OR p.face_count <> f.c
   OR ABS(p.weight - COALESCE(f.w, 0)) > 1e-6;

-- 5. 列注释
COMMENT ON COLUMN photo_person.weight IS '该人物所有人脸 score 之和(增量维护质心的权重)';
COMMENT ON COLUMN photo_person.centroid IS 'score 加权向量和 Σ(score*embedding), 未归一化, 读取时 normalize';
