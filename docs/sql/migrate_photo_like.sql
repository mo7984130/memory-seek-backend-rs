-- 照片点赞功能独立迁移脚本
-- 执行前请备份数据库

-- 1. 添加 photo_photo.like_count 字段
ALTER TABLE photo_photo ADD COLUMN IF NOT EXISTS like_count BIGINT NOT NULL DEFAULT 0;
COMMENT ON COLUMN photo_photo.like_count IS '点赞总数';

-- 2. 创建索引
CREATE INDEX IF NOT EXISTS idx_photo_like_count ON photo_photo (like_count DESC);

-- 3. 创建 photo_photo_like 表
CREATE TABLE IF NOT EXISTS photo_photo_like (
    id BIGSERIAL PRIMARY KEY,
    photo_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    -- 唯一索引：防止重复点赞
    CONSTRAINT uk_photo_user_like UNIQUE (photo_id, user_id)
);

COMMENT ON TABLE photo_photo_like IS '照片点赞记录表';
COMMENT ON COLUMN photo_photo_like.photo_id IS '照片ID';
COMMENT ON COLUMN photo_photo_like.user_id IS '点赞用户ID';
COMMENT ON COLUMN photo_photo_like.created_at IS '点赞时间';
COMMENT ON COLUMN photo_photo_like.updated_at IS '更新时间';

-- 4. 创建索引优化查询
CREATE INDEX IF NOT EXISTS idx_photo_like_photo_id ON photo_photo_like (photo_id);
CREATE INDEX IF NOT EXISTS idx_photo_like_user_id ON photo_photo_like (user_id);
CREATE INDEX IF NOT EXISTS idx_photo_like_user_photo ON photo_photo_like (user_id, photo_id);

-- 完成
SELECT 'Migration completed successfully' AS status;
