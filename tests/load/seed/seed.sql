-- tests/load/seed/seed.sql
-- 压测种子数据(幂等, 可重复执行; 通过 ON CONFLICT 保证)
-- 依赖: 已由 init.sql 建表(docs/sql/init.sql, 含 CREATE TABLE IF NOT EXISTS)
--
-- psql 变量(由 seed.sh 注入):
--   :PASS_HASH       密码哈希(argon2id, 明文统一 Test123456)
--   :AUTH_USERS      auth 压测用户数(loadtest_{1..N}@test.com)
--   :PHOTO_USERS     photo 压测用户数(loadtest_photo_{1..N}@test.com)
--   :PHOTOS_PER_USER 每个 photo 用户预置照片数
--   :FACES_PER_PERSON 每个用户归属到人物的人脸数(photo_person.face_count)
--   :PHOTO_COUNT     照片总数(PHOTO_USERS * PHOTOS_PER_USER)
--
-- id 规划(避开 init.sql 中的 admin id=1):
--   auth 压测用户  id = g + 1                  (g = 1..AUTH_USERS)
--   photo 压测用户 id = AUTH_USERS + g + 1     (g = 1..PHOTO_USERS)

-- 1. auth 压测用户
INSERT INTO auth_user (id, username, email, password, nickname, inviter)
SELECT g + 1,
       'loadtest_' || g,
       'loadtest_' || g || '@test.com',
       ':PASS_HASH',
       'LoadTest',
       0
FROM generate_series(1, :AUTH_USERS) AS g
ON CONFLICT (id) DO NOTHING;

-- 2. photo 压测用户
INSERT INTO auth_user (id, username, email, password, nickname, inviter)
SELECT (:AUTH_USERS + g + 1),
       'loadtest_photo_' || g,
       'loadtest_photo_' || g || '@test.com',
       ':PASS_HASH',
       'LoadTestPhoto',
       0
FROM generate_series(1, :PHOTO_USERS) AS g
ON CONFLICT (id) DO NOTHING;

-- 3. 照片元数据(每个 photo 用户预置若干张; file_id 唯一, 无需真实对象存储)
-- created_at 随 (u, p) 递增, 避免所有照片同刻导致分页排序退化(压测更贴近真实分布)
INSERT INTO photo_photo (user_id, name, size, width, height, mime_type, md5, file_id, created_at)
SELECT (:AUTH_USERS + u + 1),
       'seed_' || u || '_' || p,
       102400,
       400,
       300,
       'image/jpeg',
       lpad((u::bigint * 100000 + p)::text, 32, '0'),
       'seed_file_' || u || '_' || p,
       now() - interval '1 minute' * ((:PHOTO_USERS - u) * :PHOTOS_PER_USER + (:PHOTOS_PER_USER - p))
FROM generate_series(1, :PHOTO_USERS) AS u
CROSS JOIN generate_series(1, :PHOTOS_PER_USER) AS p
ON CONFLICT (file_id) DO NOTHING;

-- 4. 时间线统计(当前月份, 供 /photo/timeline/stats)
INSERT INTO photo_timeline_stat (date_str, count, anchor_time)
VALUES (to_char(now(), 'YYYY-MM'), :PHOTO_COUNT, now())
ON CONFLICT (date_str) DO UPDATE
SET count     = EXCLUDED.count,
    updated_at = now();

-- 5. 人脸(每张 seed 照片 1 张, 初始未分配; embedding 为随机 512 维)
-- 幂等: 表内仅 seed 数据, TRUNCATE RESTART IDENTITY 保证 id 从头开始
-- (photo_face/photo_person 无外键约束, 可安全 TRUNCATE)
TRUNCATE photo_face, photo_person RESTART IDENTITY;

INSERT INTO photo_face (photo_id, person_id, bbox, landmarks, score, embedding)
SELECT p.id,
       NULL,
       '[0.1,0.1,0.6,0.9]',
       '[[0.1,0.1],[0.2,0.2],[0.3,0.3],[0.4,0.4],[0.5,0.5]]',
       0.95,
       ('[' || (SELECT string_agg((random() * 2 - 1)::numeric(5,4)::text, ',')
                 FROM generate_series(1, 512)) || ']')::vector
FROM photo_photo p
WHERE p.file_id LIKE 'seed_file_%';

-- 6. 人物(每 photo 用户 1 个, cover 用其第一张照片; centroid 随机 512 维)
INSERT INTO photo_person (id, name, name_initials, cover_face_id, cover_photo_id,
                          cover_file_id, cover_face_score, cover_bbox, centroid, face_count, weight)
SELECT u,
       'Person_' || u,
       'P_' || u,
       (u - 1) * :PHOTOS_PER_USER + 1,
       (u - 1) * :PHOTOS_PER_USER + 1,
       'seed_file_' || u || '_1',
       0.95,
       '[0.1,0.1,0.6,0.9]',
       ('[' || (SELECT string_agg((random() * 2 - 1)::numeric(5,4)::text, ',')
                 FROM generate_series(1, 512)) || ']')::vector,
       :FACES_PER_PERSON,
       :FACES_PER_PERSON * 0.95
FROM generate_series(1, :PHOTO_USERS) AS u
ON CONFLICT (id) DO NOTHING;

-- 7. 每个用户前 FACES_PER_PERSON 张照片的人脸归属到对应人物
--    (person u 的照片 id 范围 [(u-1)*PHOTOS_PER_USER+1, u*PHOTOS_PER_USER])
UPDATE photo_face f
SET person_id  = ((f.photo_id - 1) / :PHOTOS_PER_USER) + 1,
    updated_at = now()
WHERE f.photo_id BETWEEN 1 AND :PHOTO_COUNT
  AND ((f.photo_id - 1) % :PHOTOS_PER_USER) + 1 <= :FACES_PER_PERSON;
