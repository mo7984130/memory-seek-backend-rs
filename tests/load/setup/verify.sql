-- tests/load/setup/verify.sql
-- 验证压测数据
SELECT
    'auth_users' AS type,
    count(*) AS count
FROM auth_user
WHERE email LIKE 'loadtest_%@test.com'
UNION ALL
SELECT
    'photo_users' AS type,
    count(*) AS count
FROM auth_user
WHERE email LIKE 'loadtest_photo_%@test.com';
