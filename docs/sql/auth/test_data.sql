-- 创建测试数据库
CREATE DATABASE IF NOT EXISTS memory_seek_test;

-- 使用测试数据库
\c memory_seek_test;

-- 创建测试用户表（如果不存在）
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password VARCHAR(255) NOT NULL,
    nickname VARCHAR(50) NOT NULL,
    avatar_file_id VARCHAR(255),
    inviter INTEGER,
    refresh_token VARCHAR(255),
    refresh_token_expire_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 清空测试数据
TRUNCATE TABLE users CASCADE;

-- 插入测试用户（100个）
-- 密码为 "Test@123"，使用 argon2id 哈希
INSERT INTO users (username, email, password, nickname)
SELECT 
    'test_user_' || i,
    'test_user_' || i || '@test.com',
    '$argon2id$v=19$m=19456,t=2,p=1$test_salt$test_hash',
    'Test User ' || i
FROM generate_series(1, 100) AS i;

-- 创建测试邀请码
CREATE TABLE IF NOT EXISTS inviter_codes (
    id SERIAL PRIMARY KEY,
    code VARCHAR(10) UNIQUE NOT NULL,
    user_id INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP
);

-- 插入测试邀请码
INSERT INTO inviter_codes (code, expires_at)
VALUES 
    ('TEST01', CURRENT_TIMESTAMP + INTERVAL '30 days'),
    ('TEST02', CURRENT_TIMESTAMP + INTERVAL '30 days'),
    ('TEST03', CURRENT_TIMESTAMP + INTERVAL '30 days');

-- 创建邮箱验证码表
CREATE TABLE IF NOT EXISTS email_verify_codes (
    id SERIAL PRIMARY KEY,
    email VARCHAR(100) NOT NULL,
    code VARCHAR(10) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP NOT NULL
);

-- 插入测试邮箱验证码
INSERT INTO email_verify_codes (email, code, expires_at)
VALUES 
    ('test_register@test.com', '123456', CURRENT_TIMESTAMP + INTERVAL '10 minutes'),
    ('test_register2@test.com', '654321', CURRENT_TIMESTAMP + INTERVAL '10 minutes');

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_inviter_codes_code ON inviter_codes(code);
CREATE INDEX IF NOT EXISTS idx_email_verify_codes_email ON email_verify_codes(email);

-- 查询测试数据
SELECT COUNT(*) as test_users_count FROM users;
SELECT * FROM inviter_codes;
SELECT * FROM email_verify_codes;
