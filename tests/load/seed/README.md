# tests/load/seed/ 种子数据

> **占位目录**: 压测种子数据的接入约定, 具体实现逻辑待后续补齐。

## 目的

CI/本地一键环境中, 被测服务面对的是一个全新 PG 实例。种子数据负责灌入压测所需的基础数据, 使场景脚本(`helpers/common.js`)中的预设账号可用:

- `auth` 场景: `loadtest_{1..AUTH_USERS}@test.com`(密码 `Test123456`), 默认 `AUTH_USERS=10000`
- `photo` 场景: `loadtest_photo_{1..PHOTO_USERS}@test.com`, 默认 `PHOTO_USERS=200`

数据量通过 `ci/env.sh` 的 `AUTH_USERS` / `PHOTO_USERS` 与 `helpers/common.js` 对齐。

## 约定(待实现)

| 内容 | 说明 |
|---|---|
| 入口 | `scripts/seed.sh`(或等价脚本), 幂等(可重复执行), 失败可重跑 |
| 数据 | 批量注册用户、插入照片元数据、集合/评论/点赞样本 |
| 对接 | `make seed` 在 `ci-up` 之后、`run-all` 之前调用 |
| 限制 | 不依赖 S3(CI 不模拟), 图片上传路径的种子数据无需真实对象存储 |
| 性能 | 批量插入(COPY/批量 insert), 避免逐条网络往返拖慢环境准备 |

## 与场景脚本的关系

`tests/load/helpers/common.js:16-17` 定义了账号基数, 种子脚本必须保证该基数内的账号均可登录。**任何一方调整基数时须同步修改。**
