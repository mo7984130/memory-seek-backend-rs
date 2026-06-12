#!/bin/bash
# 初始化测试数据库

set -e

# 数据库连接信息
DB_HOST=${DB_HOST:-localhost}
DB_PORT=${DB_PORT:-5433}
DB_USER=${DB_USER:-test}
DB_PASSWORD=${DB_PASSWORD:-test}
DB_NAME=${DB_NAME:-memory_seek_test}

# 等待数据库就绪
echo "等待数据库就绪..."
for i in {1..30}; do
    if PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c '\q' 2>/dev/null; then
        echo "数据库已就绪"
        break
    fi
    sleep 1
done

# 创建表结构
echo "创建表结构..."

# Auth 用户表
PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME << 'EOF'
CREATE TABLE IF NOT EXISTS "auth_user" (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL,
    password VARCHAR(255) NOT NULL,
    nickname VARCHAR(255) NOT NULL,
    avatar_file_id VARCHAR(2000),
    inviter BIGINT NOT NULL,
    refresh_token CHAR(32),
    refresh_token_expire_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    CONSTRAINT user_username_index UNIQUE (username),
    CONSTRAINT user_email_index UNIQUE (email)
);

COMMENT ON TABLE "auth_user" IS '用户表';
COMMENT ON COLUMN "auth_user".id IS '主键ID';
COMMENT ON COLUMN "auth_user".username IS '用户名(唯一索引)';
COMMENT ON COLUMN "auth_user".email IS '邮箱地址';
COMMENT ON COLUMN "auth_user".password IS '加密后的密码';
COMMENT ON COLUMN "auth_user".nickname IS '用户昵称';
COMMENT ON COLUMN "auth_user".avatar_file_id IS '头像文件ID';
COMMENT ON COLUMN "auth_user".inviter IS '邀请人ID';
COMMENT ON COLUMN "auth_user".refresh_token IS '刷新令牌';
COMMENT ON COLUMN "auth_user".refresh_token_expire_at IS '刷新令牌过期时间';
COMMENT ON COLUMN "auth_user".updated_at IS '更新时间';
COMMENT ON COLUMN "auth_user".created_at IS '创建时间';
EOF

echo "数据库初始化完成"
