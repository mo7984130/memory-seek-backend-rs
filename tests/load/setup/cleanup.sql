-- tests/load/setup/cleanup.sql
-- 清理压测数据
DELETE FROM auth_user WHERE email LIKE '%@test.com';
