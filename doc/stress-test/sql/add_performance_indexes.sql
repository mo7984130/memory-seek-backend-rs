-- 压测性能优化索引
-- 用于优化登录查询性能

-- 为 auth_user 表的 username 字段添加索引
-- 这个索引可以显著提升登录查询性能
CREATE INDEX IF NOT EXISTS idx_auth_user_username ON auth_user(username);

-- 验证索引是否生效
EXPLAIN ANALYZE SELECT * FROM auth_user WHERE username = 'testuser01';

-- 查看索引列表
SELECT 
    indexname, 
    indexdef 
FROM pg_indexes 
WHERE tablename = 'auth_user';
