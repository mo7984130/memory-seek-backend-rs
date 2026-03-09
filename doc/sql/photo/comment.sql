-- 照片评论表
CREATE TABLE IF NOT EXISTS photo_comment (
     id BIGSERIAL PRIMARY KEY,
     photo_id BIGINT NOT NULL,
     user_id BIGINT NOT NULL,
     content TEXT NOT NULL,
     like_count integer DEFAULT 0 NOT NULL, -- 冗余字段：点赞总数，方便展示
     created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
     updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

COMMENT ON TABLE photo_comment IS '照片评论表';
COMMENT ON COLUMN photo_comment.id IS '主键ID';
COMMENT ON COLUMN photo_comment.photo_id IS '照片ID';
COMMENT ON COLUMN photo_comment.user_id IS '评论者用户ID';
COMMENT ON COLUMN photo_comment.content IS '评论内容';
COMMENT ON COLUMN photo_comment.like_count IS '点赞总数统计';
COMMENT ON COLUMN photo_comment.created_at IS '创建时间';
COMMENT ON COLUMN photo_comment.updated_at IS '更新时间';

-- 索引：按照片和时间快速加载评论
CREATE INDEX idx_comment_photo_time ON photo_comment (photo_id, created_at DESC);
COMMENT ON INDEX idx_comment_photo_time IS '优化：按照片查询评论';

CREATE INDEX idx_comment_photo_likes ON photo_comment (photo_id, like_count DESC);
COMMENT ON INDEX idx_comment_photo_likes IS '优化：按照片查询高赞评论';

-- 评论点赞记录表
CREATE TABLE IF NOT EXISTS photo_comment_like (
    id BIGSERIAL PRIMARY KEY,
    comment_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    -- 唯一索引：防止重复点赞
    CONSTRAINT uk_comment_user_like UNIQUE (comment_id, user_id)
);

COMMENT ON TABLE photo_comment_like IS '评论点赞记录表';
COMMENT ON COLUMN photo_comment_like.comment_id IS '评论ID';
COMMENT ON COLUMN photo_comment_like.user_id IS '点赞用户ID';
COMMENT ON COLUMN photo_comment_like.created_at IS '点赞时间';
COMMENT ON COLUMN photo_comment_like.updated_at IS '更新时间';