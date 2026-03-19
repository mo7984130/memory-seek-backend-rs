-- 开启向量拓展
-- apt-get install postgresql-16-pgvector
CREATE EXTENSION IF NOT EXISTS vector;

-- 人物信息表
CREATE TABLE IF NOT EXISTS photo_face_person(
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    name_initials VARCHAR(50),
    max_score_feature_id BIGINT NOT NULL,
    max_score FLOAT4 DEFAULT 0.0 NOT NULL,
    total_photo_count BIGINT DEFAULT 0 NOT NULL, -- 总图片数
    centroid_embedding vector(512) default '[0.0]' NOT NULL, -- 中心特征向量
    total_weight_count FLOAT4 DEFAULT 0.0 NOT NULL, -- 总权重
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT uk_person_name UNIQUE (name)
);
COMMENT ON TABLE photo_face_person IS '人物信息表';
COMMENT ON COLUMN photo_face_person.id IS '人物ID';
COMMENT ON COLUMN photo_face_person.name IS '人物名称';
COMMENT ON COLUMN photo_face_person.name_initials IS '名字首字母（拼音）';
COMMENT ON COLUMN photo_face_person.max_score_feature_id IS '最大置信度特征ID';
COMMENT ON COLUMN photo_face_person.total_photo_count IS '总图片数';
COMMENT ON COLUMN photo_face_person.centroid_embedding IS '中心特征向量';
COMMENT ON COLUMN photo_face_person.total_weight_count IS '总权重';
COMMENT ON COLUMN photo_face_person.created_at IS '创建时间';
COMMENT ON COLUMN photo_face_person.updated_at IS '更新时间';

CREATE INDEX IF NOT EXISTS idx_person_name_initials ON photo_face_person (name_initials);

-- 人脸特征表
CREATE TABLE IF NOT EXISTS photo_face_feature(
    id BIGSERIAL PRIMARY KEY,
    photo_id BIGINT NOT NULL,
    person_id BIGINT NULL,
    embedding vector(512) NOT NULL,
    bbox JSONB NOT NULL,
    score FLOAT4 NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP NOT NULL
);
COMMENT ON TABLE photo_face_feature IS '人脸特征表';
COMMENT ON COLUMN photo_face_feature.id IS '特征ID';
COMMENT ON COLUMN photo_face_feature.photo_id IS '图片ID';
COMMENT ON COLUMN photo_face_feature.person_id IS '人物ID';
COMMENT ON COLUMN photo_face_feature.embedding IS '人脸特征向量';
COMMENT ON COLUMN photo_face_feature.bbox IS '人脸边界框';
COMMENT ON COLUMN photo_face_feature.score IS '置信度';
COMMENT ON COLUMN photo_face_feature.created_at IS '创建时间';
COMMENT ON COLUMN photo_face_feature.updated_at IS '更新时间';
-- 该索引先按人物分组，再在组内按分数倒序排列
CREATE INDEX IF NOT EXISTS idx_feature_person_score
    ON photo_face_feature (person_id, score DESC);

-- 创建向量索引
CREATE INDEX idx_photo_face_feature_embedding ON photo_face_feature USING hnsw (embedding vector_cosine_ops);
-- 创建人物索引
CREATE INDEX idx_photo_face_feature_person_id ON photo_face_feature USING btree (person_id);



