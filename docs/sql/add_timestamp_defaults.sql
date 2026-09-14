-- 给所有实体的 created_at / updated_at 列补 DB 默认值 CURRENT_TIMESTAMP。
--
-- 背景:
--   entity 已声明 `#[sea_orm(default_expr = "Expr::current_timestamp()")]`,
--   但 sea-orm 的 schema sync 只增不改(见 SchemaBuilder::sync 文档):
--   已存在的表不会被 ALTER, 因此老库需要本脚本对齐; 新库由 sync 直接建出默认值。
--
-- 说明:
--   updated_at 的默认值只在 INSERT 生效; UPDATE 时仍需业务显式写入(本项目一直如此)。
--
-- 用法:
--   psql "$DATABASE_URL" -f docs/sql/add_timestamp_defaults.sql

-- created_at: 全部实体
ALTER TABLE "audit_event"            ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "auth_user"              ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_photo"            ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_collection"       ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_collection_photo" ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_comment"          ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_comment_like"     ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_face"             ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_person"           ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_photo_like"       ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_timeline_stat"    ALTER COLUMN "created_at" SET DEFAULT CURRENT_TIMESTAMP;

-- updated_at: 仅带该列的实体
ALTER TABLE "auth_user"           ALTER COLUMN "updated_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_photo"         ALTER COLUMN "updated_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_collection"    ALTER COLUMN "updated_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_comment"       ALTER COLUMN "updated_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_face"          ALTER COLUMN "updated_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_person"        ALTER COLUMN "updated_at" SET DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE "photo_timeline_stat" ALTER COLUMN "updated_at" SET DEFAULT CURRENT_TIMESTAMP;
