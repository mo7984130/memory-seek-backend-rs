create table if not exists "auth_user"
(
    id                      BIGSERIAL PRIMARY KEY ,
    username                varchar(255) not null,
    email                   varchar(255) not null,
    password                varchar(255) not null,
    nickname                varchar(255) not null,
    avatar_file_id          varchar(2000) null,
    inviter                 bigint not null,
    refresh_token           char(32) null,
    refresh_token_expire_at TIMESTAMPTZ null,
    updated_at              TIMESTAMPTZ DEFAULT NOW() not null,
    created_at              TIMESTAMPTZ DEFAULT NOW() not null,
    constraint user_username_index unique (username)
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