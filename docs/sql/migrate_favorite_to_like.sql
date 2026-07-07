-- 迁移脚本：将"我喜欢"收藏夹中的照片同步到照片点赞表，然后删除"我喜欢"收藏夹
-- 执行前请备份数据库！

BEGIN;

-- 1. 将"我喜欢"收藏夹中的照片插入到 photo_photo_like（跳过已存在的）
INSERT INTO photo_photo_like (photo_id, user_id, created_at, updated_at)
SELECT DISTINCT
    pcp.photo_id,
    pcp.user_id,
    pcp.created_at,
    NOW()
FROM photo_collection_photo pcp
INNER JOIN photo_collection pc ON pc.id = pcp.collection_id
WHERE pc.is_favorite = true
ON CONFLICT (photo_id, user_id) DO NOTHING;

-- 2. 更新 photo_photo.like_count（重新统计）
UPDATE photo_photo
SET like_count = (
    SELECT COUNT(*)
    FROM photo_photo_like
    WHERE photo_photo_like.photo_id = photo_photo.id
),
updated_at = NOW()
WHERE id IN (
    SELECT DISTINCT pcp.photo_id
    FROM photo_collection_photo pcp
    INNER JOIN photo_collection pc ON pc.id = pcp.collection_id
    WHERE pc.is_favorite = true
);

-- 3. 删除"我喜欢"收藏夹中的照片关联
DELETE FROM photo_collection_photo
WHERE collection_id IN (
    SELECT id FROM photo_collection WHERE is_favorite = true
);

-- 4. 删除"我喜欢"收藏夹
DELETE FROM photo_collection
WHERE is_favorite = true;

-- 5. 删除 is_favorite 列
ALTER TABLE photo_collection DROP COLUMN IF EXISTS is_favorite;

COMMIT;

SELECT 'Migration: favorite -> like completed successfully' AS status;
