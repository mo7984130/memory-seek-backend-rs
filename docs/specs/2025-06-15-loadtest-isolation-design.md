# 压测环境隔离方案

## 背景

当前压测方案直接往生产数据库插入测试数据（seed.sql），存在两个问题：

1. 数据量太小（1000 用户），无法模拟真实场景下的查询性能
2. 污染生产数据库，即使清理也有残留风险

## 设计目标

- 生产服务零影响（不重启、不改配置）
- 数据库和 Redis 完全隔离
- 可配置数据量级
- 压测结束干净清理
- 本地开发和 CI 共用同一套脚本

## 架构

```
生产服务 (port 7984, memory_seek,       redis/0)  ← 不受影响
临时服务 (port 7985, memory_seek_loadtest, redis/1)  ← k6 压测目标
```

## SQL 一致性

**`docs/sql/init.sql` 是唯一的表结构来源。**

压测数据库通过 `init.sql` 建表，不维护独立的 SQL 文件。这保证：
- 压测用的表结构和生产完全一致
- 修改实体时必须同步更新 `init.sql`，否则集成测试会失败（测试也用 `init.sql` 建库）

一致性保障链：
```
修改 entities/ → 必须同步 docs/sql/init.sql → CI 测试用 init.sql 建库 → 不一致则测试失败
```

## 流程

```
1. SSH → CREATE DATABASE memory_seek_loadtest
2. SSH → psql -d memory_seek_loadtest -f docs/sql/init.sql（建表）
3. SSH → psql -d memory_seek_loadtest -f seed.sql（填充可配置量级的数据）
4. SSH → 创建 config.loadtest.json（指向测试库, port 7985, redis/1）
5. SSH → 后台启动 ./server（MEMORY_SEEK_CONFIG_PATH=config.loadtest.json）
6. 等待临时服务就绪（health check 轮询）
7. 本地/CI → k6 run → http://server:7985
8. SSH → kill 临时服务进程
9. SSH → DROP DATABASE memory_seek_loadtest
10. SSH → rm config.loadtest.json
```

## 数据量配置

通过环境变量控制 seed 数量：

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| AUTH_USERS | 10000 | auth 模块测试用户数 |
| PHOTOS | 100000 | 照片记录数 |
| PHOTO_USERS | 20 | photo 模块测试用户数 |

数据量建议：

| 场景 | AUTH_USERS | PHOTOS | seed 耗时 |
|------|-----------|--------|----------|
| 快速验证 | 1,000 | 10,000 | ~10s |
| 日常压测 | 10,000 | 100,000 | ~1min |
| 严格压测 | 100,000 | 1,000,000 | ~5-10min |

## k6 脚本设计

目录结构与 `domains/` 保持一致：

```
tests/load/scripts/
├── auth/
│   └── auth_service.js      # POST /auth/register, POST /auth/login, POST /auth/token
├── user/
│   └── user_service.js      # GET /user/me, PATCH /user/nickname, PATCH /user/password, POST /user/logout
└── photo/
    ├── photo_service.js      # POST /photo/upload, GET /photo/, GET /photo/{id}, DELETE /photo/{id}
    ├── comment_service.js    # POST /comment/{photo_id}, GET /comment/{photo_id}, DELETE /comment/{photo_id}/{id}, like/unlike
    ├── collection_service.js # POST /collection/, GET /collection/, PATCH /collection/{id}, DELETE /collection/{id}
    └── collection_photo_service.js # POST /collection/{id}/photo, DELETE /collection/{id}/photo
```

所有 k6 脚本的 `BASE_URL` 必须通过 `__ENV.BASE_URL` 显式传入，不设默认值，未设置时直接报错退出。

## 文件变更

### 新建文件

#### `tests/load/setup/seed.sql`

仅负责插入数据（不建表，建表由 `docs/sql/init.sql` 完成）。
通过 psql 变量控制数据量，无硬编码数量：

```sql
-- 前置条件：数据库已通过 docs/sql/init.sql 建表
-- 使用方式: psql -d memory_seek_loadtest -v auth_users=10000 -v photo_users=20 -v photos=100000 -f seed.sql

-- auth 压测用户
INSERT INTO auth_user (username, email, password, nickname, inviter, created_at)
SELECT
    'loadtest_' || i,
    'loadtest_' || i || '@test.com',
    '$2b$12$LJ3m4ys3Lk0TSwHjnF4oR.K3VJxqfVYqxSy3TqFG3YfP0z3bGHXBe',
    'LoadTest User ' || i,
    'TEST01',
    NOW()
FROM generate_series(1, :'auth_users'::int) AS i
ON CONFLICT (email) DO NOTHING;

-- photo 压测用户
INSERT INTO auth_user (username, email, password, nickname, inviter, created_at)
SELECT
    'loadtest_photo_' || i,
    'loadtest_photo_' || i || '@test.com',
    '$2b$12$LJ3m4ys3Lk0TSwHjnF4oR.K3VJxqfVYqxSy3TqFG3YfP0z3bGHXBe',
    'Photo User ' || i,
    'TEST01',
    NOW()
FROM generate_series(1, :'photo_users'::int) AS i
ON CONFLICT (email) DO NOTHING;

-- 照片记录（分配给 photo 用户）
INSERT INTO photo_photo (user_id, name, size, width, height, mime_type, md5, file_id, created_at)
SELECT
    (SELECT id FROM auth_user WHERE email = 'loadtest_photo_' || ((i % :photo_users::int) + 1) || '@test.com'),
    'photo_' || i || '.jpg',
    (random() * 5000000 + 100000)::bigint,
    (random() * 3000 + 1000)::int,
    (random() * 2000 + 800)::int,
    'image/jpeg',
    md5(random()::text),
    'loadtest_file_' || i,
    NOW() - (random() * interval '365 days')
FROM generate_series(1, :'photos'::int) AS i;

-- 时间线统计
INSERT INTO photo_timeline_stat (date_str, count, anchor_time)
SELECT
    to_char(d, 'YYYY-MM'),
    (random() * 1000 + 100)::bigint,
    d
FROM generate_series(
    date_trunc('month', NOW() - interval '12 months'),
    date_trunc('month', NOW()),
    interval '1 month'
) AS d
ON CONFLICT (date_str) DO NOTHING;
```

#### `tests/load/setup/verify.sql`

```sql
SELECT 'auth_users' AS type, count(*) AS count
FROM auth_user WHERE email LIKE 'loadtest_%@test.com'
UNION ALL
SELECT 'photo_users' AS type, count(*) AS count
FROM auth_user WHERE email LIKE 'loadtest_photo_%@test.com'
UNION ALL
SELECT 'photos' AS type, count(*) AS count
FROM photo_photo WHERE file_id LIKE 'loadtest_file_%';
```

#### `tests/load/setup/loadtest-config.json`

临时服务配置模板，部署时通过 sed 替换变量：

```json
{
    "server": {
        "host": "0.0.0.0",
        "port": 7985
    },
    "database": {
        "url": "postgres://DB_USER:DB_PASS@localhost:5432/memory_seek_loadtest",
        "max_connections": 50
    },
    "redis": {
        "url": "redis://127.0.0.1:6379/1",
        "max_connections": 10
    },
    "smtp": {
        "server": "localhost",
        "port": 1025,
        "username": "",
        "password": "",
        "from_email": "test@loadtest.local",
        "from_name": "LoadTest"
    },
    "s3": {
        "endpoint": "S3_ENDPOINT",
        "access_key": "S3_ACCESS_KEY",
        "secret_key": "S3_SECRET_KEY",
        "region": "S3_REGION",
        "bucket": "S3_BUCKET",
        "public_url": "S3_PUBLIC_URL"
    },
    "token_cipher": {
        "key": "TOKEN_KEY",
        "salt": "TOKEN_SALT"
    }
}
```

#### `tests/load/setup/start-loadtest.sh`

临时服务启动脚本（服务器端执行）：

```bash
#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SERVER_BIN="${SERVER_BIN:-./server}"
CONFIG_SRC="${SCRIPT_DIR}/loadtest-config.json"
CONFIG_DST="/tmp/config.loadtest.json"
PID_FILE="/tmp/loadtest-server.pid"
PORT=7985

# 参数: DB_USER DB_PASS S3_ENDPOINT S3_ACCESS_KEY S3_SECRET_KEY S3_REGION S3_BUCKET S3_PUBLIC_URL TOKEN_KEY TOKEN_SALT
DB_USER="$1"
DB_PASS="$2"
S3_ENDPOINT="$3"
S3_ACCESS_KEY="$4"
S3_SECRET_KEY="$5"
S3_REGION="$6"
S3_BUCKET="$7"
S3_PUBLIC_URL="$8"
TOKEN_KEY="$9"
TOKEN_SALT="${10}"

# 替换配置模板变量
sed -e "s|DB_USER|${DB_USER}|g" \
    -e "s|DB_PASS|${DB_PASS}|g" \
    -e "s|S3_ENDPOINT|${S3_ENDPOINT}|g" \
    -e "s|S3_ACCESS_KEY|${S3_ACCESS_KEY}|g" \
    -e "s|S3_SECRET_KEY|${S3_SECRET_KEY}|g" \
    -e "s|S3_REGION|${S3_REGION}|g" \
    -e "s|S3_BUCKET|${S3_BUCKET}|g" \
    -e "s|S3_PUBLIC_URL|${S3_PUBLIC_URL}|g" \
    -e "s|TOKEN_KEY|${TOKEN_KEY}|g" \
    -e "s|TOKEN_SALT|${TOKEN_SALT}|g" \
    "$CONFIG_SRC" > "$CONFIG_DST"

# 启动临时服务
cd "$(dirname "$SERVER_BIN")"
MEMORY_SEEK_CONFIG_PATH="$CONFIG_DST" "$SERVER_BIN" &
echo $! > "$PID_FILE"
echo "Loadtest server started, PID=$(cat "$PID_FILE"), port=$PORT"

# 等待服务就绪
for i in $(seq 1 30); do
    if curl -sf "http://localhost:$PORT/" > /dev/null 2>&1; then
        echo "Server ready"
        exit 0
    fi
    sleep 1
done
echo "ERROR: Server failed to start within 30s"
exit 1
```

#### `tests/load/setup/stop-loadtest.sh`

临时服务停止脚本：

```bash
#!/bin/bash
set -euo pipefail

PID_FILE="/tmp/loadtest-server.pid"

if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        kill "$PID"
        echo "Loadtest server stopped (PID=$PID)"
    fi
    rm -f "$PID_FILE"
fi
```

### 修改文件

#### `tests/load/Makefile`

重写 Makefile，支持完整的隔离式压测流程：

- `make seed` — SSH 创建数据库 + 建表 + 填充数据
- `make start-server` — SSH 启动临时服务
- `make stop-server` — SSH 停止临时服务
- `make teardown` — SSH 停止服务 + 删除数据库 + 清理配置（DROP DATABASE，不需要 cleanup.sql）
- `make loadtest` — 完整流程（seed → start → k6 run scripts/ → teardown）
- `make loadtest-quick` — 快速验证（少量数据 + 单个 service 脚本）

#### `tests/load/config/remote.json`

仅保留 SSH 连接信息和数据库凭据，不再需要 baseUrl（从 `--out` 或环境变量获取）：

```json
{
    "ssh": {
        "host": "YOUR_SERVER_IP",
        "port": 22,
        "user": "root"
    },
    "db": {
        "user": "postgres",
        "pass": "YOUR_DB_PASS",
        "name": "memory_seek_loadtest"
    },
    "server_bin": "/opt/memory-seek/server",
    "s3": {
        "endpoint": "YOUR_S3_ENDPOINT",
        "access_key": "YOUR_S3_ACCESS_KEY",
        "secret_key": "YOUR_S3_SECRET_KEY",
        "region": "YOUR_S3_REGION",
        "bucket": "YOUR_S3_BUCKET",
        "public_url": "YOUR_S3_PUBLIC_URL"
    },
    "token_cipher": {
        "key": "YOUR_TOKEN_KEY",
        "salt": "YOUR_TOKEN_SALT"
    }
}
```

#### `.github/workflows/release.yml`

loadtest job 改为使用新的隔离流程：

1. SSH 执行 seed（创建数据库 + 建表 + 填充数据）
2. SSH 启动临时服务
3. k6 run scripts/auth/auth_service.js, scripts/user/user_service.js, scripts/photo/*.js（连接 server:7985）
4. SSH 执行 teardown（停止服务 + 删库 + 清理配置）

新增 Secrets（test Environment）：

| Secret | 说明 |
|--------|------|
| LOADTEST_DB_PASS | 数据库密码 |
| LOADTEST_S3_ENDPOINT | S3 端点 |
| LOADTEST_S3_ACCESS_KEY | S3 Access Key |
| LOADTEST_S3_SECRET_KEY | S3 Secret Key |
| LOADTEST_S3_REGION | S3 Region |
| LOADTEST_S3_BUCKET | S3 Bucket |
| LOADTEST_S3_PUBLIC_URL | S3 Public URL |
| LOADTEST_TOKEN_KEY | Token 加密 key |
| LOADTEST_TOKEN_SALT | Token 加密 salt |

## 数据库分离

压测使用独立的 PostgreSQL 数据库，和生产库完全隔离：

| 资源 | 生产 | 压测 |
|------|------|------|
| 数据库名 | memory_seek | memory_seek_loadtest |
| PostgreSQL 用户 | 复用同一个用户 | 复用同一个用户 |
| 建表来源 | docs/sql/init.sql | docs/sql/init.sql（同一份） |

使用同一个 PostgreSQL 用户是因为压测数据库在测试结束后直接 DROP，不需要额外的权限隔离。
如果服务器上 PostgreSQL 配置了 peer 认证，SSH 用户就是数据库用户，无需密码。

## 隔离保证

| 资源 | 生产 | 压测 | 隔离方式 |
|------|------|------|----------|
| 数据库 | memory_seek | memory_seek_loadtest | 独立数据库 |
| Redis | redis/0 | redis/1 | 分库 |
| 服务端口 | 7984 | 7985 | 不同端口 |
| 配置文件 | config.json | config.loadtest.json | 独立文件 |
| S3/MinIO | 共享 | 共享 | 测试数据用 loadtest_ 前缀区分 |
| SMTP | 共享 | 不需要 | 压测不发邮件 |

## 错误处理

- 任何步骤失败，执行 teardown（停止服务 + 删库 + 清理配置）
- Makefile 中用 `trap` 确保异常退出也能 teardown
- GitHub Actions 中 teardown step 用 `if: always()` 确保执行

## 测试验证

1. 本地执行 `make loadtest-quick` 验证完整流程
2. 检查压测结束后服务器上无残留（无临时进程、无测试数据库、无临时配置）
3. 确认生产服务未受影响（port 7984 正常响应）
