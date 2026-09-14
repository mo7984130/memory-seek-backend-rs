-- 给 photo_comment.like_count 补 DB 默认值 0。
--
-- 背景:
--   CommentMapper::insert 未设置该列(靠 ..Default::default() 省略), 而该列
--   NOT NULL 且无默认值 → 发布评论报 23502 not-null violation。
--   entity 已补 #[sea_orm(default_value = 0)]; schema sync 只增不改,
--   老库需执行本脚本对齐(新库由 sync 直接建出默认值)。
--
-- 用法:
--   psql "$DATABASE_URL" -f docs/sql/add_comment_like_count_default.sql

ALTER TABLE "photo_comment" ALTER COLUMN "like_count" SET DEFAULT 0;
