-- 1. 收藏夹主表 (文件夹)
CREATE TABLE IF NOT EXISTS photo_collection (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    photo_count BIGINT DEFAULT 0 NOT NULL,
    cover_file_id VARCHAR,
    is_favorite BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- 表与字段注释
COMMENT ON TABLE photo_collection IS '用户收藏夹表';
COMMENT ON COLUMN photo_collection.id IS '主键ID';
COMMENT ON COLUMN photo_collection.user_id IS '创建者用户ID';
COMMENT ON COLUMN photo_collection.name IS '收藏夹名称';
COMMENT ON COLUMN photo_collection.description IS '收藏夹详细描述';
COMMENT ON COLUMN photo_collection.photo_count IS '逻辑字段：统计该收藏夹下的图片总数';
COMMENT ON COLUMN photo_collection.cover_file_id IS '收藏夹封面图的文件ID';
COMMENT ON COLUMN photo_collection.is_favorite IS '是否为我喜欢';
COMMENT ON COLUMN photo_collection.created_at IS '创建时间';
COMMENT ON COLUMN photo_collection.updated_at IS '更新时间';

-- 2. 收藏关系表 (多对多)
CREATE TABLE IF NOT EXISTS photo_collection_photo (
    id BIGSERIAL PRIMARY KEY,
    collection_id BIGINT NOT NULL,
    photo_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,

    -- 唯一索引：防止同一张图在同一收藏夹重复
    CONSTRAINT uk_collection_photo UNIQUE (collection_id, photo_id)
);

-- 表与字段注释
COMMENT ON TABLE photo_collection_photo IS '收藏夹与图片的关联关系表';
COMMENT ON COLUMN photo_collection_photo.id IS '主键ID';
COMMENT ON COLUMN photo_collection_photo.collection_id IS '关联的收藏夹ID';
COMMENT ON COLUMN photo_collection_photo.photo_id IS '关联的图片ID';
COMMENT ON COLUMN photo_collection_photo.user_id IS '所属用户ID（用于越权检查）';
COMMENT ON COLUMN photo_collection_photo.created_at IS '收藏时间/创建时间';
COMMENT ON COLUMN photo_collection_photo.updated_at IS '关系更新时间';

-- 3. 物理索引 (优化查询)
CREATE INDEX idx_collection_user_id ON photo_collection(user_id);
CREATE INDEX idx_fp_collection_id_created_id ON photo_collection_photo(collection_id, created_at DESC, id DESC);
COMMENT ON INDEX idx_collection_user_id IS '优化：按用户查询收藏夹列表';
COMMENT ON INDEX idx_fp_collection_id_created_id IS '优化：按收藏时间倒序查询收藏夹内容（复合游标）';