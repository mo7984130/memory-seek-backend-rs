-- ============================================================
-- photo_person.name_initials 可空化(对齐实体 Option<String>)
-- ============================================================
-- 背景: photo_person 实体中 name_initials 为 Option<String>,
--       且当前代码(NewPerson)从不写入该字段;
--       旧建表 DDL 为 NOT NULL, 导致全新库执行 full_scan 插入人物时
--       "null value in column name_initials" 报错。
-- 本脚本: 将存量数据库该列放松为可空(init.sql 已同步为 NULL)。
-- ============================================================

-- 1. 放松非空约束(幂等, 已是可空时无副作用)
ALTER TABLE photo_person ALTER COLUMN name_initials DROP NOT NULL;

-- 2. (可选)若历史数据中有空字符串占位, 可统一置为 NULL, 语义更贴近 Option
-- UPDATE photo_person SET name_initials = NULL WHERE name_initials = '';
