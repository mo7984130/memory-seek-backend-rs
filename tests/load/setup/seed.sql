-- tests/load/setup/seed.sql
-- 预置压测数据
-- 前置条件：数据库已通过 docs/sql/init.sql 建表
-- 使用方式: psql -d memory_seek_loadtest -v auth_users=10000 -v photo_users=200 -v photos=100000 -f seed.sql
-- 密码: Test123456 (bcrypt hash)

-- auth 压测用户
INSERT INTO auth_user (username, email, password, nickname, inviter, created_at)
SELECT
    'loadtest_' || i,
    'loadtest_' || i || '@test.com',
    '$argon2id$v=19$m=16384,t=2,p=1$zcGSKX21GtoXbkIRxMLPXQ$QyhhvsEdkENJXKJS9LBaphiQX5nHQcc+w/MGdwUwYzQ',
    'LoadTest User ' || i,
    1,
    NOW()
FROM generate_series(1, :'auth_users'::int) AS i
ON CONFLICT (email) DO NOTHING;

-- photo 压测用户
INSERT INTO auth_user (username, email, password, nickname, inviter, created_at)
SELECT
    'loadtest_photo_' || i,
    'loadtest_photo_' || i || '@test.com',
    '$argon2id$v=19$m=16384,t=2,p=1$zcGSKX21GtoXbkIRxMLPXQ$QyhhvsEdkENJXKJS9LBaphiQX5nHQcc+w/MGdwUwYzQ',
    'Photo User ' || i,
    1,
    NOW()
FROM generate_series(1, :'photo_users'::int) AS i
ON CONFLICT (email) DO NOTHING;

-- 照片记录（分配给 photo 用户）
INSERT INTO photo_photo (user_id, name, size, width, height, mime_type, md5, file_id, created_at)
SELECT
    (SELECT id FROM auth_user WHERE email = 'loadtest_photo_' || ((i % :'photo_users'::int) + 1) || '@test.com'),
    'photo_' || i || '.jpg',
    (random() * 5000000 + 100000)::bigint,
    (random() * 3000 + 1000)::int,
    (random() * 2000 + 800)::int,
    'image/jpeg',
    md5(random()::text),
    'loadtest_file_' || i,
    NOW() - (random() * interval '365 days')
FROM generate_series(1, :'photos'::int) AS i;

-- 时间线统计
INSERT INTO photo_timeline_stat (date_str, count, anchor_time)
SELECT
    to_char(d, 'YYYY-MM'),
    (random() * 1000 + 100)::bigint,
    d
FROM generate_series(
    date_trunc('month', NOW() - interval '12 months'),
    date_trunc('month', NOW()),
    interval '1 month'
) AS d
ON CONFLICT (date_str) DO NOTHING;
