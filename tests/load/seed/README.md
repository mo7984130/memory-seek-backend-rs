# tests/load/seed/ 种子数据

为 CI/本地一键环境灌入压测所需基础数据。已实现, 幂等可重复执行。

## 组成

| 文件 | 作用 |
|---|---|
| `seed.sh` | 入口: 建表(init.sql) → schema 对齐 → 灌数据(seed.sql) → 汇总校验 |
| `seed.sql` | 灌入压测账号 / 照片元数据 / 时间线统计(ON CONFLICT 幂等) |
| `schema_align.sql` | 补齐 `docs/sql/init.sql` 与当前代码不一致的列(如 `photo_collection.cover_photo_id`) |

执行入口: `make seed`(依赖 compose 中 postgres 服务已启动)。数据量通过 `ci/env.sh` 的 `AUTH_USERS` / `PHOTO_USERS` / `PHOTOS_PER_USER` 覆盖。

## 数据规划

- **auth 用户**: `loadtest_{1..AUTH_USERS}@test.com`, id = g+1(避开 init.sql 的 admin id=1)
- **photo 用户**: `loadtest_photo_{1..PHOTO_USERS}@test.com`, id = AUTH_USERS+g+1
- **照片**: 每个 photo 用户 `PHOTOS_PER_USER` 张(`photo_photo` 元数据, `file_id` 唯一, 无需真实对象存储)
- **时间线统计**: 当前月份的 `photo_timeline_stat`
- **密码**: 统一 `Test123456`(argon2id, 与 `common::utils::HashAlgorithm` 参数一致)

## 与场景脚本的约定

- 账号基数与 `tests/load/helpers/common.js:16-17` 的 `AUTH_USERS` / `PHOTO_USERS` 对齐, **任何一方调整须同步**。
- 场景依赖的照片来自种子数据: `list_photos` / `timeline_stats` 直接读 PG; comment / collection_photo / comment_like 场景通过 `list_photos(1)` 取照片 ID 操作。

## 已知限制

- 不依赖 S3(CI 不模拟): 种子照片无真实对象存储, 上传(`photo_service` 场景)仅本地连真实 OSS 时可跑。
- `init.sql` 存在与代码不同步的列, 由 `schema_align.sql` 在压测环境补齐(生产 schema 治理不在压测体系范围内)。
