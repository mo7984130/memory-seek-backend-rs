-- tests/load/setup/seed.sql
-- 预置压测用户数据
-- 密码: Test123456 (bcrypt hash)

-- auth 压测用户 (1000 个)
INSERT INTO auth_user (username, email, password, nickname, inviter, created_at)
SELECT
    'loadtest_' || i,
    'loadtest_' || i || '@test.com',
    '$2b$12$LJ3m4ys3Lk0TSwHjnF4oR.K3VJxqfVYqxSy3TqFG3YfP0z3bGHXBe',
    'LoadTest User ' || i,
    'TEST01',
    NOW()
FROM generate_series(1, 1000) AS i
ON CONFLICT (email) DO NOTHING;

-- photo 压测用户 (20 个)
INSERT INTO auth_user (username, email, password, nickname, inviter, created_at)
SELECT
    'loadtest_photo_' || i,
    'loadtest_photo_' || i || '@test.com',
    '$2b$12$LJ3m4ys3Lk0TSwHjnF4oR.K3VJxqfVYqxSy3TqFG3YfP0z3bGHXBe',
    'Photo User ' || i,
    'TEST01',
    NOW()
FROM generate_series(1, 20) AS i
ON CONFLICT (email) DO NOTHING;
