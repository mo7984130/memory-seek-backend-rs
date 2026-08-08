-- ============================================================
-- photo_person / photo_face 增量维护字段迁移(人脸归属转移重构)
-- ============================================================
-- 用途:
--   1. photo_face.score 从 REAL(float4)统一为 DOUBLE PRECISION(float8):
--      - pgvector 只提供 vector * float8 运算符, 无 vector * real,
--        统一后质心回填/增量公式无需显式转型;
--      - 与 photo_person.weight(float8) 类型对齐, 增量维护全程 f64。
--   2. photo_person 新增 weight 列, 供 change_face_belonging / merge_person
--      增量维护质心;
--   3. 同步把 centroid 语义从「无权简单平均(归一化)」改为
--      「score 加权向量和 Σ(score*embedding), 未归一化」。
-- 适用: 已有存量数据的数据库; 全新库直接用 init.sql, 无需本脚本。
-- 版本说明: pgvector 的 `*` 运算符只有 `vector * vector`(逐元素乘),
--   没有 `vector * float8` 标量乘法, 也没有 `avg(vector)` 向量聚合
--   (已在 0.8.5 上验证: SELECT ... FROM pg_operator WHERE oprname='*')。
--   因此 centroid 回填统一用「向量下标 + string_agg 拼字面量」方式,
--   不依赖任何向量算术/聚合, 所有 pgvector 版本通用。
-- 注意: 第 1 步会重写 photo_face 表(ACCESS EXCLUSIVE 锁), 数据量大时请在
--       维护窗口执行。
-- ============================================================

-- 0. 前置探测(只读, 不修改数据): 下标可用则回填可执行
--    报错则说明下标在你的版本也不可用, 需要应用层回填(见计划书备选方案)
SELECT (embedding)[1] AS subscript_probe FROM photo_face LIMIT 1;

-- 1. photo_face.score 统一为 float8(先于回填, 使下方语句无需显式转型)
ALTER TABLE photo_face ALTER COLUMN score TYPE DOUBLE PRECISION;

-- 2. photo_person 加列(有默认值, 存量行直接可用)
ALTER TABLE photo_person ADD COLUMN IF NOT EXISTS weight DOUBLE PRECISION NOT NULL DEFAULT 0;

-- 3. 回填 weight: 该人物所有人脸 score 之和(纯 SQL, 无 vector 运算, 所有版本通用)
UPDATE photo_person p
SET weight = COALESCE(sub.w, 0)
FROM (SELECT person_id, SUM(score) AS w
      FROM photo_face WHERE person_id IS NOT NULL GROUP BY person_id) sub
WHERE sub.person_id = p.id;

-- 4. 回填 centroid: score 加权和(旧值为无权简单平均, 与增量公式不兼容, 必须回填)
--    SUM(score * embedding[ord]) = Σ(score * embedding) 的逐分量累加;
--    再用 string_agg 拼成 '[v1,v2,...]' 文本, 经 text→vector 转换落库。
--    中间行数 = 人脸数 × 512, 数据量大时较慢, 但只跑一次。
UPDATE photo_person p
SET centroid = sub.wsum_text::vector
FROM (
    SELECT person_id,
           '[' || string_agg(acc::text, ',' ORDER BY ord) || ']' AS wsum_text
    FROM (
        SELECT f.person_id, ord, SUM(f.score * f.embedding[ord::int]) AS acc
        FROM photo_face f
        CROSS JOIN LATERAL generate_series(1, 512) AS g(ord)
        WHERE f.person_id IS NOT NULL
        GROUP BY f.person_id, ord
    ) inner
    GROUP BY person_id
) sub
WHERE sub.person_id = p.id;

-- 5. 校验一致性(期望 0 行; 浮点比较带容差)
SELECT p.id, p.face_count, p.weight
FROM photo_person p
LEFT JOIN (SELECT person_id, COUNT(*) AS c, SUM(score) AS w
           FROM photo_face WHERE person_id IS NOT NULL GROUP BY person_id) f
       ON f.person_id = p.id
WHERE f.c IS NULL
   OR p.face_count <> f.c
   OR ABS(p.weight - COALESCE(f.w, 0)) > 1e-6;

-- 6. 列注释
COMMENT ON COLUMN photo_person.weight IS '该人物所有人脸 score 之和(增量维护质心的权重)';
COMMENT ON COLUMN photo_person.centroid IS 'score 加权向量和 Σ(score*embedding), 未归一化, 读取时 normalize';
COMMENT ON COLUMN photo_face.score IS '人脸检测置信度(REAL 统一为 DOUBLE PRECISION, 与 weight/质心公式对齐)';
