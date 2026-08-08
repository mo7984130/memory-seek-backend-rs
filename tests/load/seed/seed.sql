-- tests/load/seed/seed.sql
-- 压测种子数据(幂等, 可重复执行; 通过 ON CONFLICT 保证)
-- 依赖: 已由 init.sql 建表(docs/sql/init.sql, 含 CREATE TABLE IF NOT EXISTS)
--
-- psql 变量(由 seed.sh 注入):
--   :PASS_HASH       密码哈希(argon2id, 明文统一 Test123456)
--   :AUTH_USERS      auth 压测用户数(loadtest_{1..N}@test.com)
--   :PHOTO_USERS     photo 压测用户数(loadtest_photo_{1..N}@test.com)
--   :PHOTOS_PER_USER 每个 photo 用户预置照片数
--   :PHOTO_COUNT     时间线统计中的照片总数(PHOTO_USERS * PHOTOS_PER_USER)
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
INSERT INTO photo_photo (user_id, name, size, width, height, mime_type, md5, file_id)
SELECT (:AUTH_USERS + u + 1),
       'seed_' || u || '_' || p,
       102400,
       400,
       300,
       'image/jpeg',
       lpad((u::bigint * 100000 + p)::text, 32, '0'),
       'seed_file_' || u || '_' || p
FROM generate_series(1, :PHOTO_USERS) AS u
CROSS JOIN generate_series(1, :PHOTOS_PER_USER) AS p
ON CONFLICT (file_id) DO NOTHING;

-- 4. 时间线统计(当前月份, 供 /photo/timeline/stats)
INSERT INTO photo_timeline_stat (date_str, count, anchor_time)
VALUES (to_char(now(), 'YYYY-MM'), :PHOTO_COUNT, now())
ON CONFLICT (date_str) DO UPDATE
SET count     = EXCLUDED.count,
    updated_at = now();
