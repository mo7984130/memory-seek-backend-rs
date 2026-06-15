# 压测环境隔离方案 — 实施计划

> **For agentic workers:** REQUIRED_SUBAGENTS = 1 | PARALLEL = false | 并行会破坏顺序依赖。

## TL;DR

> **快速摘要:** 将压测基础设施从"直连生产数据库"改为"每次压测创建独立临时数据库 + 临时服务进程"，k6 脚本按 domains 目录结构重组。
>
> **产出物:**
> - 新建 8 个文件（k6 脚本 ×6，启动脚本 ×2）
> - 重写 4 个文件（common.js, seed.sql, Makefile, remote.json）
> - 删除 1 个文件（cleanup.sql）
> - 修改 1 个文件（release.yml）
>
> **预估耗时:** 30-45 分钟
> **并行执行:** NO — 必须串行
> **关键路径:** Task 1 → Task 2 → Task 3 → Task 4 → Task 5

---

## Context

### 原始请求

将压测基础设施从本地 Podman Compose + 生产数据库模式改为隔离式远程压测：每次测试创建独立数据库和临时服务进程，k6 脚本按 domains 目录结构重组。

### 技术栈

- **k6** — 负载测试框架
- **PostgreSQL** — CREATE/DROP DATABASE 实现数据库隔离
- **Redis** — 分库隔离（redis/0 生产，redis/1 压测）
- **SSH** — 远程执行数据库和服务管理命令
- **GitHub Actions** — CI/CD 自动化
- **Axum** — Rust HTTP 框架（被测服务）

### 完整路由映射

从源码 `domains/*/src/controllers/*.rs` 和 `server/src/setup/domains/mod.rs` 中提取：

| 模块 | 路径前缀 | 认证 | 路由 |
|------|----------|------|------|
| Auth | 无 | 公开 | POST /login, POST /register, POST /token, POST /verification-codes |
| User | /user | 受保护 | GET /me, PATCH /nickname, PATCH /password, POST /logout, PUT /avatar, POST /inviter-code, POST /batch |
| Photo | /photo | 受保护 | GET /, POST / (upload), DELETE /, POST /check-existence |
| Photo | /photo | 公开 | GET /{token} (get image) |
| Collection | /photo/collections | 受保护 | GET /, POST /, PATCH /{id}, DELETE /{id} |
| CollectionPhoto | /photo/collections | 受保护 | GET /{collection_id}/photos, POST /{collection_id}/photos, DELETE /{collection_id}/photos, DELETE /{collection_id}/photos/{photo_id} |
| Comment | /photo/comment | 受保护 | GET /{photo_id}, POST /{photo_id}, DELETE /{photo_id}/{comment_id} |
| CommentLike | /photo/comment | 受保护 | POST /{photo_id}/{comment_id}/like, DELETE /{photo_id}/{comment_id}/like |
| Timeline | /photo/timeline | 受保护 | GET /stats |

---

## Tasks

---

### Task 1: 重写 `tests/load/helpers/common.js`

**文件:** `tests/load/helpers/common.js` (重写)

**做什么:**
- 移除 `BASE_URL` 的默认值，未设置时直接报错退出
- 添加 `AUTH_USERS` 和 `PHOTO_USERS` 环境变量读取（与 seed.sql 对齐）
- 移除硬编码的 1000/20 数字

**完整新文件内容:**

```javascript
// tests/load/helpers/common.js
// k6 公共函数

import http from 'k6/http';

// BASE_URL 必须通过 -e BASE_URL=... 显式传入
const BASE_URL = __ENV.BASE_URL;
if (!BASE_URL) {
  throw new Error('BASE_URL is required. Pass via: k6 run -e BASE_URL=http://host:port script.js');
}

// 数据量配置（与 seed.sql 的 psql 变量对齐）
const AUTH_USERS = parseInt(__ENV.AUTH_USERS || '10000');
const PHOTO_USERS = parseInt(__ENV.PHOTO_USERS || '20');

/**
 * 用户登录并返回 accessToken
 * @param {string} account - 邮箱账号
 * @param {string} password - 密码
 * @returns {string|null} accessToken 或 null
 */
export function login(account, password) {
  const res = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password,
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  if (res.status === 200) {
    return res.json('data.accessToken');
  }

  console.error(`Login failed for ${account}: ${res.status} ${res.body}`);
  return null;
}

/**
 * 生成 auth 测试用户凭据
 * @param {number} vuId - VU ID
 * @returns {{ account: string, password: string }}
 */
export function getTestUserCredentials(vuId) {
  const userId = (vuId % AUTH_USERS) + 1;
  return {
    account: `loadtest_${userId}@test.com`,
    password: 'Test123456',
  };
}

/**
 * 生成 photo 测试用户凭据
 * @param {number} vuId - VU ID
 * @returns {{ account: string, password: string }}
 */
export function getPhotoUserCredentials(vuId) {
  const userId = (vuId % PHOTO_USERS) + 1;
  return {
    account: `loadtest_photo_${userId}@test.com`,
    password: 'Test123456',
  };
}

/**
 * 创建带 Authorization 头的请求头
 * @param {string} token - accessToken
 * @returns {Object} headers
 */
export function authHeaders(token) {
  return {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${token}`,
  };
}

export { BASE_URL };
```

**验证:** `grep -n '||' tests/load/helpers/common.js` — 不应有 `|| 'http://...` 的默认值模式。

---

### Task 2: 重写 `tests/load/setup/seed.sql`

**文件:** `tests/load/setup/seed.sql` (重写)

**做什么:**
- 使用 psql 变量 `:'auth_users'`、`:'photo_users'`、`:'photos'` 替代硬编码数字
- 添加照片记录和时间线统计的 INSERT
- 仅负责插入数据，不建表（建表由 `docs/sql/init.sql` 完成）

**完整新文件内容:**

```sql
-- tests/load/setup/seed.sql
-- 预置压测数据
-- 前置条件：数据库已通过 docs/sql/init.sql 建表
-- 使用方式: psql -d memory_seek_loadtest -v auth_users=10000 -v photo_users=20 -v photos=100000 -f seed.sql
-- 密码: Test123456 (bcrypt hash)

-- auth 压测用户
INSERT INTO auth_user (username, email, password, nickname, inviter, created_at)
SELECT
    'loadtest_' || i,
    'loadtest_' || i || '@test.com',
    '$2b$12$LJ3m4ys3Lk0TSwHjnF4oR.K3VJxqfVYqxSy3TqFG3YfP0z3bGHXBe',
    'LoadTest User ' || i,
    1,
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
    1,
    NOW()
FROM generate_series(1, :'photo_users'::int) AS i
ON CONFLICT (email) DO NOTHING;

-- 照片记录（分配给 photo 用户）
INSERT INTO photo_photo (user_id, name, size, width, height, mime_type, md5, file_id, created_at)
SELECT
    (SELECT id FROM auth_user WHERE email = 'loadtest_photo_' || ((i % :'photo_users'::int) + 1) || '@test.com'),
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

**注意:** `inviter` 字段从 `'TEST01'` 改为 `1`，因为 `inviter` 是 `BIGINT` 类型（见 `docs/sql/init.sql` 第 17 行）。

**验证:** `grep -c "generate_series" tests/load/setup/seed.sql` — 应返回 4。

---

### Task 3: 删除 `tests/load/setup/cleanup.sql`

**文件:** `tests/load/setup/cleanup.sql` (删除)

**做什么:** 删除此文件。压测结束后直接 DROP DATABASE，不需要清理 SQL。

**验证:** `test ! -f tests/load/setup/cleanup.sql` — 文件不应存在。

---

### Task 4: 重写 `tests/load/setup/verify.sql`

**文件:** `tests/load/setup/verify.sql` (重写)

**做什么:** 添加 photo_photo 计数验证。

**完整新文件内容:**

```sql
-- tests/load/setup/verify.sql
-- 验证压测数据
SELECT
    'auth_users' AS type,
    count(*) AS count
FROM auth_user
WHERE email LIKE 'loadtest_%@test.com'
UNION ALL
SELECT
    'photo_users' AS type,
    count(*) AS count
FROM auth_user
WHERE email LIKE 'loadtest_photo_%@test.com'
UNION ALL
SELECT
    'photos' AS type,
    count(*) AS count
FROM photo_photo
WHERE file_id LIKE 'loadtest_file_%';
```

**验证:** `grep -c "UNION ALL" tests/load/setup/verify.sql` — 应返回 2。

---

### Task 5: 创建配置模板和脚本

#### Task 5a: 创建 `tests/load/setup/loadtest-config.json`

**文件:** `tests/load/setup/loadtest-config.json` (新建)

**做什么:** 临时服务配置模板，部署时通过 sed 替换变量占位符。

**完整文件内容:**

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

**验证:** `jq .server.port tests/load/setup/loadtest-config.json` — 应输出 `7985`。

---

#### Task 5b: 创建 `tests/load/setup/start-loadtest.sh`

**文件:** `tests/load/setup/start-loadtest.sh` (新建)

**做什么:** 服务器端执行的启动脚本。创建数据库、建表、填充数据、生成配置、启动临时服务、等待就绪。

**完整文件内容:**

```bash
#!/bin/bash
set -euo pipefail

# ============================================================
# 压测临时服务启动脚本（服务器端执行）
# 用法: ./start-loadtest.sh <DB_USER> <DB_PASS> <AUTH_USERS> <PHOTO_USERS> <PHOTOS> \
#         <S3_ENDPOINT> <S3_ACCESS_KEY> <S3_SECRET_KEY> <S3_REGION> <S3_BUCKET> <S3_PUBLIC_URL> \
#         <TOKEN_KEY> <TOKEN_SALT> [SERVER_BIN]
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DB_USER="$1"
DB_PASS="$2"
AUTH_USERS="${3:-10000}"
PHOTO_USERS="${4:-20}"
PHOTOS="${5:-100000}"
S3_ENDPOINT="$6"
S3_ACCESS_KEY="$7"
S3_SECRET_KEY="$8"
S3_REGION="$9"
S3_BUCKET="${10}"
S3_PUBLIC_URL="${11}"
TOKEN_KEY="${12}"
TOKEN_SALT="${13}"
SERVER_BIN="${14:-./server}"

CONFIG_SRC="${SCRIPT_DIR}/loadtest-config.json"
CONFIG_DST="/tmp/config.loadtest.json"
PID_FILE="/tmp/loadtest-server.pid"
DB_NAME="memory_seek_loadtest"
PORT=7985

echo "=== 压测环境启动 ==="

# 1. 创建压测数据库
echo "📦 创建数据库 ${DB_NAME}..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d postgres \
  -c "DROP DATABASE IF EXISTS ${DB_NAME};" \
  -c "CREATE DATABASE ${DB_NAME};"

# 2. 建表（使用 init.sql 作为唯一表结构来源）
echo "📋 建表..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d "${DB_NAME}" \
  -f "${SCRIPT_DIR}/../../../docs/sql/init.sql"

# 3. 填充数据
echo "🌱 填充数据 (auth_users=${AUTH_USERS}, photo_users=${PHOTO_USERS}, photos=${PHOTOS})..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d "${DB_NAME}" \
  -v auth_users="${AUTH_USERS}" \
  -v photo_users="${PHOTO_USERS}" \
  -v photos="${PHOTOS}" \
  -f "${SCRIPT_DIR}/seed.sql"

# 4. 生成配置文件
echo "⚙️  生成配置..."
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

# 5. 启动临时服务
echo "🚀 启动临时服务 (port=${PORT})..."
cd "$(dirname "$SERVER_BIN")"
MEMORY_SEEK_CONFIG_PATH="$CONFIG_DST" "$SERVER_BIN" &
echo $! > "$PID_FILE"
echo "   PID=$(cat "$PID_FILE")"

# 6. 等待服务就绪
echo "⏳ 等待服务就绪..."
for i in $(seq 1 30); do
    if curl -sf "http://localhost:${PORT}/login" -o /dev/null 2>&1; then
        echo "✅ 临时服务就绪"
        exit 0
    fi
    sleep 1
done
echo "❌ 服务启动超时 (30s)"
exit 1
```

**验证:** `bash -n tests/load/setup/start-loadtest.sh` — 语法检查通过。

---

#### Task 5c: 创建 `tests/load/setup/stop-loadtest.sh`

**文件:** `tests/load/setup/stop-loadtest.sh` (新建)

**做什么:** 服务器端执行的停止脚本。停止临时服务、删除数据库、清理配置文件。

**完整文件内容:**

```bash
#!/bin/bash
set -euo pipefail

# ============================================================
# 压测临时服务停止 + 清理脚本（服务器端执行）
# 用法: ./stop-loadtest.sh <DB_USER> <DB_PASS>
# ============================================================

DB_USER="$1"
DB_PASS="$2"
PID_FILE="/tmp/loadtest-server.pid"
CONFIG_DST="/tmp/config.loadtest.json"
DB_NAME="memory_seek_loadtest"

echo "=== 压测环境清理 ==="

# 1. 停止临时服务
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        kill "$PID"
        echo "🛑 临时服务已停止 (PID=${PID})"
    else
        echo "⚠️  临时服务已不存在 (PID=${PID})"
    fi
    rm -f "$PID_FILE"
else
    echo "⚠️  未找到 PID 文件"
fi

# 2. 删除压测数据库
echo "🗑️  删除数据库 ${DB_NAME}..."
PGPASSWORD="${DB_PASS}" psql -h localhost -U "${DB_USER}" -d postgres \
  -c "DROP DATABASE IF EXISTS ${DB_NAME};" 2>/dev/null || true

# 3. 清理配置文件
if [ -f "$CONFIG_DST" ]; then
    rm -f "$CONFIG_DST"
    echo "🧹 配置文件已清理"
fi

echo "✅ 清理完成"
```

**验证:** `bash -n tests/load/setup/stop-loadtest.sh` — 语法检查通过。

---

### Task 6: 创建 k6 脚本（按 domains 目录结构）

#### Task 6a: 创建 `tests/load/scripts/auth/auth_service.js`

**文件:** `tests/load/scripts/auth/auth_service.js` (新建)

**做什么:** 认证模块压测脚本，覆盖登录、注册、token 刷新。

**完整文件内容:**

```javascript
// tests/load/scripts/auth/auth_service.js
// 认证模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getTestUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const loginErrorRate = new Rate('login_errors');
const loginDuration = new Trend('login_duration');
const registerErrorRate = new Rate('register_errors');
const registerDuration = new Trend('register_duration');
const tokenErrorRate = new Rate('token_refresh_errors');
const tokenDuration = new Trend('token_refresh_duration');

export const options = {
  stages: [
    { duration: '30s', target: 50 },
    { duration: '1m', target: 50 },
    { duration: '30s', target: 100 },
    { duration: '1m', target: 100 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    login_errors: ['rate<0.01'],
    register_errors: ['rate<0.05'],
    token_refresh_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getTestUserCredentials(__VU);

  // 1. 登录
  const loginRes = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password,
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  check(loginRes, {
    'login status is 200': (r) => r.status === 200,
    'login has token': (r) => r.json('data.accessToken') !== undefined,
  });

  loginErrorRate.add(loginRes.status !== 200);
  loginDuration.add(loginRes.timings.duration);

  if (loginRes.status !== 200) {
    console.error(`Login failed: ${loginRes.body}`);
    return;
  }

  const token = loginRes.json('data.accessToken');
  const refreshToken = loginRes.json('data.refreshToken');

  sleep(0.5);

  // 2. Token 刷新
  if (refreshToken) {
    const tokenRes = http.post(`${BASE_URL}/token`, JSON.stringify({
      refreshToken,
    }), {
      headers: { 'Content-Type': 'application/json' },
    });

    check(tokenRes, {
      'token refresh status is 200': (r) => r.status === 200,
    });

    tokenErrorRate.add(tokenRes.status !== 200);
    tokenDuration.add(tokenRes.timings.duration);
  }

  sleep(0.5);

  // 3. 访问受保护接口验证 token 有效性
  const meRes = http.get(`${BASE_URL}/user/me`, {
    headers: authHeaders(token),
  });

  check(meRes, {
    'me status is 200': (r) => r.status === 200,
  });

  sleep(1);
}
```

**验证:** `node --check tests/load/scripts/auth/auth_service.js 2>&1 || true` — 无语法错误（k6 模块不可用时会报 import 错误，这是正常的）。

---

#### Task 6b: 创建 `tests/load/scripts/user/user_service.js`

**文件:** `tests/load/scripts/user/user_service.js` (新建)

**做什么:** 用户模块压测脚本，覆盖获取用户信息、修改昵称、修改密码、登出。

**完整文件内容:**

```javascript
// tests/load/scripts/user/user_service.js
// 用户模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getTestUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const profileErrorRate = new Rate('profile_errors');
const profileDuration = new Trend('profile_duration');
const updateErrorRate = new Rate('update_errors');
const updateDuration = new Trend('update_duration');

export const options = {
  stages: [
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 50 },
    { duration: '1m', target: 50 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    profile_errors: ['rate<0.01'],
    update_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getTestUserCredentials(__VU);

  // 登录获取 token
  const token = login(account, password);
  if (!token) return;

  const headers = authHeaders(token);

  // 1. 获取当前用户信息
  const meRes = http.get(`${BASE_URL}/user/me`, { headers });

  check(meRes, {
    'get me status is 200': (r) => r.status === 200,
    'get me has data': (r) => r.json('data') !== undefined,
  });

  profileErrorRate.add(meRes.status !== 200);
  profileDuration.add(meRes.timings.duration);

  if (meRes.status !== 200) {
    console.error(`Get me failed: ${meRes.body}`);
    return;
  }

  sleep(0.5);

  // 2. 修改昵称
  const nicknameRes = http.patch(`${BASE_URL}/user/nickname`, JSON.stringify({
    nickname: `Updated ${__VU} ${Date.now()}`,
  }), { headers });

  check(nicknameRes, {
    'change nickname status is 200': (r) => r.status === 200,
  });

  updateErrorRate.add(nicknameRes.status !== 200);
  updateDuration.add(nicknameRes.timings.duration);

  sleep(0.5);

  // 3. 修改密码（使用相同密码，避免影响后续测试）
  const passwordRes = http.patch(`${BASE_URL}/user/password`, JSON.stringify({
    oldPassword: password,
    newPassword: password,
  }), { headers });

  check(passwordRes, {
    'change password status is 200': (r) => r.status === 200,
  });

  updateErrorRate.add(passwordRes.status !== 200);
  updateDuration.add(passwordRes.timings.duration);

  sleep(0.5);

  // 4. 登出
  const logoutRes = http.post(`${BASE_URL}/user/logout`, null, { headers });

  check(logoutRes, {
    'logout status is 200': (r) => r.status === 200,
  });

  sleep(1);
}
```

---

#### Task 6c: 创建 `tests/load/scripts/photo/photo_service.js`

**文件:** `tests/load/scripts/photo/photo_service.js` (新建)

**做什么:** 照片模块压测脚本，覆盖上传、列表查询、详情查询、删除。

**完整文件内容:**

```javascript
// tests/load/scripts/photo/photo_service.js
// 照片模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { SharedArray } from 'k6/data';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const uploadErrorRate = new Rate('upload_errors');
const uploadDuration = new Trend('upload_duration');
const queryErrorRate = new Rate('query_errors');
const queryDuration = new Trend('query_duration');

// 共享图片数据
const testImage = new SharedArray('test-image', function () {
  return [open('../../fixtures/test.jpg', 'b')];
});

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 10 },
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<1000'],
    http_req_failed: ['rate<0.01'],
    upload_errors: ['rate<0.01'],
    query_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getPhotoUserCredentials(__VU);

  // 登录获取 token
  const token = login(account, password);
  if (!token) return;

  const headers = authHeaders(token);

  // 1. 上传照片
  const formData = {
    file: http.file(testImage[0], 'test.jpg', 'image/jpeg'),
  };

  const uploadRes = http.post(`${BASE_URL}/photo/`, formData, {
    headers: { 'Authorization': `Bearer ${token}` },
  });

  check(uploadRes, {
    'upload status is 200': (r) => r.status === 200,
  });

  uploadErrorRate.add(uploadRes.status !== 200);
  uploadDuration.add(uploadRes.timings.duration);

  if (uploadRes.status !== 200) {
    console.error(`Upload failed: ${uploadRes.body}`);
    return;
  }

  sleep(0.5);

  // 2. 查询照片列表
  const listRes = http.get(`${BASE_URL}/photo/?size=20`, { headers });

  check(listRes, {
    'list status is 200': (r) => r.status === 200,
    'list has data': (r) => r.json('data') !== undefined,
  });

  queryErrorRate.add(listRes.status !== 200);
  queryDuration.add(listRes.timings.duration);

  sleep(0.5);

  // 3. 查询时间线统计
  const timelineRes = http.get(`${BASE_URL}/photo/timeline/stats`, { headers });

  check(timelineRes, {
    'timeline stats status is 200': (r) => r.status === 200,
  });

  queryErrorRate.add(timelineRes.status !== 200);
  queryDuration.add(timelineRes.timings.duration);

  sleep(1);
}
```

---

#### Task 6d: 创建 `tests/load/scripts/photo/comment_service.js`

**文件:** `tests/load/scripts/photo/comment_service.js` (新建)

**做什么:** 评论模块压测脚本，覆盖发表评论、查询评论、删除评论、点赞/取消点赞。

**完整文件内容:**

```javascript
// tests/load/scripts/photo/comment_service.js
// 评论模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const commentErrorRate = new Rate('comment_errors');
const commentDuration = new Trend('comment_duration');

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 10 },
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    comment_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getPhotoUserCredentials(__VU);

  // 登录获取 token
  const token = login(account, password);
  if (!token) return;

  const headers = authHeaders(token);

  // 先获取照片列表，取一个 photo_id
  const listRes = http.get(`${BASE_URL}/photo/?size=1`, { headers });
  if (listRes.status !== 200 || !listRes.json('data') || listRes.json('data').length === 0) {
    console.error('No photos available for commenting');
    return;
  }

  const photoId = listRes.json('data')[0].id;
  if (!photoId) {
    console.error('Could not get photo ID');
    return;
  }

  sleep(0.3);

  // 1. 发表评论
  const createRes = http.post(`${BASE_URL}/photo/comment/${photoId}`, JSON.stringify({
    content: `LoadTest comment from VU${__VU} at ${Date.now()}`,
  }), { headers });

  check(createRes, {
    'create comment status is 200': (r) => r.status === 200,
  });

  commentErrorRate.add(createRes.status !== 200);
  commentDuration.add(createRes.timings.duration);

  if (createRes.status !== 200) {
    console.error(`Create comment failed: ${createRes.body}`);
    return;
  }

  const commentId = createRes.json('data.id');

  sleep(0.3);

  // 2. 查询评论列表
  const listCommentRes = http.get(`${BASE_URL}/photo/comment/${photoId}?size=10`, { headers });

  check(listCommentRes, {
    'list comments status is 200': (r) => r.status === 200,
  });

  commentErrorRate.add(listCommentRes.status !== 200);
  commentDuration.add(listCommentRes.timings.duration);

  sleep(0.3);

  // 3. 点赞评论
  if (commentId) {
    const likeRes = http.post(`${BASE_URL}/photo/comment/${photoId}/${commentId}/like`, null, { headers });

    check(likeRes, {
      'like comment status is 200': (r) => r.status === 200,
    });

    commentErrorRate.add(likeRes.status !== 200);

    sleep(0.3);

    // 4. 取消点赞
    const unlikeRes = http.del(`${BASE_URL}/photo/comment/${photoId}/${commentId}/like`, null, { headers });

    check(unlikeRes, {
      'unlike comment status is 200': (r) => r.status === 200,
    });

    commentErrorRate.add(unlikeRes.status !== 200);

    sleep(0.3);

    // 5. 删除评论
    const deleteRes = http.del(`${BASE_URL}/photo/comment/${photoId}/${commentId}`, null, { headers });

    check(deleteRes, {
      'delete comment status is 200': (r) => r.status === 200,
    });

    commentErrorRate.add(deleteRes.status !== 200);
  }

  sleep(1);
}
```

---

#### Task 6e: 创建 `tests/load/scripts/photo/collection_service.js`

**文件:** `tests/load/scripts/photo/collection_service.js` (新建)

**做什么:** 收藏夹模块压测脚本，覆盖创建、查询列表、更新信息、删除收藏夹。

**完整文件内容:**

```javascript
// tests/load/scripts/photo/collection_service.js
// 收藏夹模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const collectionErrorRate = new Rate('collection_errors');
const collectionDuration = new Trend('collection_duration');

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 10 },
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    collection_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getPhotoUserCredentials(__VU);

  // 登录获取 token
  const token = login(account, password);
  if (!token) return;

  const headers = authHeaders(token);

  // 1. 创建收藏夹
  const createRes = http.post(`${BASE_URL}/photo/collections/`, JSON.stringify({
    name: `Test Collection ${__VU} ${Date.now()}`,
    description: 'LoadTest collection',
  }), { headers });

  check(createRes, {
    'create collection status is 200': (r) => r.status === 200,
  });

  collectionErrorRate.add(createRes.status !== 200);
  collectionDuration.add(createRes.timings.duration);

  if (createRes.status !== 200) {
    console.error(`Create collection failed: ${createRes.body}`);
    return;
  }

  const collectionId = createRes.json('data.id');

  sleep(0.3);

  // 2. 查询收藏夹列表
  const listRes = http.get(`${BASE_URL}/photo/collections/?size=10`, { headers });

  check(listRes, {
    'list collections status is 200': (r) => r.status === 200,
  });

  collectionErrorRate.add(listRes.status !== 200);
  collectionDuration.add(listRes.timings.duration);

  sleep(0.3);

  // 3. 更新收藏夹信息
  if (collectionId) {
    const updateRes = http.patch(`${BASE_URL}/photo/collections/${collectionId}`, JSON.stringify({
      name: `Updated Collection ${__VU}`,
      description: 'Updated description',
    }), { headers });

    check(updateRes, {
      'update collection status is 200': (r) => r.status === 200,
    });

    collectionErrorRate.add(updateRes.status !== 200);
    collectionDuration.add(updateRes.timings.duration);

    sleep(0.3);

    // 4. 删除收藏夹
    const deleteRes = http.del(`${BASE_URL}/photo/collections/${collectionId}`, null, { headers });

    check(deleteRes, {
      'delete collection status is 200': (r) => r.status === 200,
    });

    collectionErrorRate.add(deleteRes.status !== 200);
  }

  sleep(1);
}
```

---

#### Task 6f: 创建 `tests/load/scripts/photo/collection_photo_service.js`

**文件:** `tests/load/scripts/photo/collection_photo_service.js` (新建)

**做什么:** 收藏夹-照片关联压测脚本，覆盖添加照片到收藏夹、查询收藏夹照片、移除照片。

**完整文件内容:**

```javascript
// tests/load/scripts/photo/collection_photo_service.js
// 收藏夹-照片关联压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const cpErrorRate = new Rate('collection_photo_errors');
const cpDuration = new Trend('collection_photo_duration');

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 10 },
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    collection_photo_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getPhotoUserCredentials(__VU);

  // 登录获取 token
  const token = login(account, password);
  if (!token) return;

  const headers = authHeaders(token);

  // 获取照片列表，取一个 photo_id
  const photoListRes = http.get(`${BASE_URL}/photo/?size=1`, { headers });
  if (photoListRes.status !== 200 || !photoListRes.json('data') || photoListRes.json('data').length === 0) {
    console.error('No photos available');
    return;
  }
  const photoId = photoListRes.json('data')[0].id;

  // 创建收藏夹
  const createColRes = http.post(`${BASE_URL}/photo/collections/`, JSON.stringify({
    name: `CP Test ${__VU} ${Date.now()}`,
  }), { headers });

  if (createColRes.status !== 200) {
    console.error(`Create collection failed: ${createColRes.body}`);
    return;
  }
  const collectionId = createColRes.json('data.id');

  sleep(0.3);

  // 1. 添加照片到收藏夹
  const addRes = http.post(`${BASE_URL}/photo/collections/${collectionId}/photos`, JSON.stringify({
    photoIds: [photoId],
  }), { headers });

  check(addRes, {
    'add photos to collection status is 200': (r) => r.status === 200,
  });

  cpErrorRate.add(addRes.status !== 200);
  cpDuration.add(addRes.timings.duration);

  sleep(0.3);

  // 2. 查询收藏夹照片列表
  const listRes = http.get(`${BASE_URL}/photo/collections/${collectionId}/photos?size=10`, { headers });

  check(listRes, {
    'list collection photos status is 200': (r) => r.status === 200,
  });

  cpErrorRate.add(listRes.status !== 200);
  cpDuration.add(listRes.timings.duration);

  sleep(0.3);

  // 3. 从收藏夹移除单张照片
  const removeRes = http.del(`${BASE_URL}/photo/collections/${collectionId}/photos/${photoId}`, null, { headers });

  check(removeRes, {
    'remove photo from collection status is 200': (r) => r.status === 200,
  });

  cpErrorRate.add(removeRes.status !== 200);

  sleep(0.3);

  // 清理：删除收藏夹
  http.del(`${BASE_URL}/photo/collections/${collectionId}`, null, { headers });

  sleep(1);
}
```

---

### Task 7: 重写 `tests/load/Makefile`

**文件:** `tests/load/Makefile` (重写)

**做什么:** 完整重写 Makefile，支持隔离式压测流程。

**完整新文件内容:**

```makefile
# tests/load/Makefile
# 隔离式压测自动化入口

CONFIG_FILE ?= config/remote.json

# 从配置文件读取连接信息
SSH_HOST    ?= $(shell jq -r .ssh.host $(CONFIG_FILE))
SSH_PORT    ?= $(shell jq -r .ssh.port $(CONFIG_FILE))
SSH_USER    ?= $(shell jq -r .ssh.user $(CONFIG_FILE))
DB_USER     ?= $(shell jq -r .db.user $(CONFIG_FILE))
DB_PASS     ?= $(shell jq -r .db.pass $(CONFIG_FILE))
SERVER_BIN  ?= $(shell jq -r .server_bin $(CONFIG_FILE))
S3_ENDPOINT ?= $(shell jq -r .s3.endpoint $(CONFIG_FILE))
S3_ACCESS_KEY ?= $(shell jq -r .s3.access_key $(CONFIG_FILE))
S3_SECRET_KEY ?= $(shell jq -r .s3.secret_key $(CONFIG_FILE))
S3_REGION   ?= $(shell jq -r .s3.region $(CONFIG_FILE))
S3_BUCKET   ?= $(shell jq -r .s3.bucket $(CONFIG_FILE))
S3_PUBLIC_URL ?= $(shell jq -r .s3.public_url $(CONFIG_FILE))
TOKEN_KEY   ?= $(shell jq -r .token_cipher.key $(CONFIG_FILE))
TOKEN_SALT  ?= $(shell jq -r .token_cipher.salt $(CONFIG_FILE))

# 数据量配置
AUTH_USERS  ?= 10000
PHOTO_USERS ?= 20
PHOTOS      ?= 100000

# k6 目标服务器（临时服务端口）
BASE_URL    ?= http://$(SSH_HOST):7985

SSH_CMD = ssh -p $(SSH_PORT) $(SSH_USER)@$(SSH_HOST)

# 脚本路径（相对于 tests/load/）
SETUP_DIR = setup

.PHONY: help seed start-server stop-server teardown loadtest loadtest-quick verify

help: ## 显示帮助信息
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

seed: ## SSH 创建数据库 + 建表 + 填充数据
	@echo "=== Seed: 创建压测数据库 ==="
	$(SSH_CMD) "bash -s" < $(SETUP_DIR)/start-loadtest.sh \
		$(DB_USER) $(DB_PASS) $(AUTH_USERS) $(PHOTO_USERS) $(PHOTOS) \
		$(S3_ENDPOINT) $(S3_ACCESS_KEY) $(S3_SECRET_KEY) $(S3_REGION) $(S3_BUCKET) $(S3_PUBLIC_URL) \
		$(TOKEN_KEY) $(TOKEN_SALT) $(SERVER_BIN)

stop-server: ## SSH 停止临时服务 + 删库 + 清理配置
	@echo "=== Teardown: 清理压测环境 ==="
	$(SSH_CMD) "bash -s" < $(SETUP_DIR)/stop-loadtest.sh $(DB_USER) $(DB_PASS)

teardown: stop-server ## 同 stop-server

loadtest: seed ## 完整压测流程 (seed → k6 run all → teardown)
	@echo "=== Loadtest: 开始全量压测 ==="
	@trap 'make teardown' EXIT; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/auth/auth_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/user/user_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/photo/photo_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/photo/comment_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/photo/collection_service.js; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=$(AUTH_USERS) -e PHOTO_USERS=$(PHOTO_USERS) \
		scripts/photo/collection_photo_service.js
	@echo "=== Loadtest: 全量压测完成 ==="

loadtest-quick: ## 快速压测 (少量数据, 仅 auth 模块)
	@echo "=== Quick Loadtest ==="
	@AUTH_USERS=100 PHOTO_USERS=5 PHOTOS=100 make seed
	@trap 'make teardown' EXIT; \
	k6 run -e BASE_URL=$(BASE_URL) -e AUTH_USERS=100 -e PHOTO_USERS=5 \
		--duration 30s --vus 10 \
		scripts/auth/auth_service.js
	@echo "=== Quick Loadtest 完成 ==="

verify: ## 验证压测数据量
	@echo "=== 验证压测数据 ==="
	$(SSH_CMD) "PGPASSWORD=$(DB_PASS) psql -h localhost -U $(DB_USER) -d memory_seek_loadtest" \
		< $(SETUP_DIR)/verify.sql
```

**验证:** `make -n -f tests/load/Makefile help` — 应能解析 Makefile。

---

### Task 8: 重写 `tests/load/config/remote.json`

**文件:** `tests/load/config/remote.json` (重写)

**做什么:** 新格式，包含 SSH、DB、S3、token_cipher 配置。

**完整新文件内容:**

```json
{
    "ssh": {
        "host": "YOUR_SERVER_IP",
        "port": 22,
        "user": "root"
    },
    "db": {
        "user": "postgres",
        "pass": "YOUR_DB_PASS"
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

**验证:** `jq .ssh.host tests/load/config/remote.json` — 应输出 `"YOUR_SERVER_IP"`。

---

### Task 9: 修改 `.github/workflows/release.yml`

**文件:** `.github/workflows/release.yml` (修改 loadtest job)

**做什么:** 重写 loadtest job，使用新的隔离流程。

**loadtest job 完整替换内容（替换从 `# ── 第三步：压测` 到文件末尾的部分）:**

```yaml
    # ── 第三步：压测 ──────────────────────────────
    loadtest:
        name: Load Test
        runs-on: ubuntu-latest
        needs: deploy
        environment: test

        steps:
            - name: Checkout
              uses: actions/checkout@v4

            - name: Install k6
              run: |
                  sudo gpg -k
                  sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg \
                    --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
                  echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" \
                    | sudo tee /etc/apt/sources.list.d/k6.list
                  sudo apt-get update && sudo apt-get install -y k6

            - name: Setup SSH
              run: |
                  mkdir -p ~/.ssh
                  echo "${{ secrets.LOADTEST_SSH_KEY }}" > ~/.ssh/deploy_key
                  chmod 600 ~/.ssh/deploy_key
                  ssh-keyscan -p ${{ secrets.SERVER_PORT }} ${{ secrets.SERVER_HOST }} >> ~/.ssh/known_hosts

            - name: Seed & Start loadtest server
              run: |
                  ssh -i ~/.ssh/deploy_key -p ${{ secrets.SERVER_PORT }} \
                    ${{ secrets.SERVER_USER }}@${{ secrets.SERVER_HOST }} \
                    "bash -s" < tests/load/setup/start-loadtest.sh \
                    "${{ secrets.LOADTEST_DB_USER }}" \
                    "${{ secrets.LOADTEST_DB_PASS }}" \
                    "10000" "20" "100000" \
                    "${{ secrets.LOADTEST_S3_ENDPOINT }}" \
                    "${{ secrets.LOADTEST_S3_ACCESS_KEY }}" \
                    "${{ secrets.LOADTEST_S3_SECRET_KEY }}" \
                    "${{ secrets.LOADTEST_S3_REGION }}" \
                    "${{ secrets.LOADTEST_S3_BUCKET }}" \
                    "${{ secrets.LOADTEST_S3_PUBLIC_URL }}" \
                    "${{ secrets.LOADTEST_TOKEN_KEY }}" \
                    "${{ secrets.LOADTEST_TOKEN_SALT }}" \
                    "/opt/memory-seek-server/server"

            - name: Run load tests
              working-directory: tests/load
              env:
                  BASE_URL: http://${{ secrets.SERVER_HOST }}:7985
              run: |
                  k6 run -e BASE_URL=$BASE_URL scripts/auth/auth_service.js
                  k6 run -e BASE_URL=$BASE_URL scripts/user/user_service.js
                  k6 run -e BASE_URL=$BASE_URL scripts/photo/photo_service.js
                  k6 run -e BASE_URL=$BASE_URL scripts/photo/comment_service.js
                  k6 run -e BASE_URL=$BASE_URL scripts/photo/collection_service.js
                  k6 run -e BASE_URL=$BASE_URL scripts/photo/collection_photo_service.js

            - name: Teardown loadtest environment
              if: always()
              run: |
                  ssh -i ~/.ssh/deploy_key -p ${{ secrets.SERVER_PORT }} \
                    ${{ secrets.SERVER_USER }}@${{ secrets.SERVER_HOST }} \
                    "bash -s" < tests/load/setup/stop-loadtest.sh \
                    "${{ secrets.LOADTEST_DB_USER }}" \
                    "${{ secrets.LOADTEST_DB_PASS }}"

            - name: Cleanup SSH key
              if: always()
              run: rm -f ~/.ssh/deploy_key
```

**需要新增的 GitHub Secrets（test Environment）:**

| Secret | 说明 |
|--------|------|
| LOADTEST_DB_USER | 数据库用户名 |
| LOADTEST_DB_PASS | 数据库密码 |
| LOADTEST_S3_ENDPOINT | S3 端点 |
| LOADTEST_S3_ACCESS_KEY | S3 Access Key |
| LOADTEST_S3_SECRET_KEY | S3 Secret Key |
| LOADTEST_S3_REGION | S3 Region |
| LOADTEST_S3_BUCKET | S3 Bucket |
| LOADTEST_S3_PUBLIC_URL | S3 Public URL |
| LOADTEST_TOKEN_KEY | Token 加密 key |
| LOADTEST_TOKEN_SALT | Token 加密 salt |

**验证:** `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"` — YAML 语法正确。

---

## 验证

### 执行验证命令

```bash
# 1. 检查所有新文件存在
test -f tests/load/helpers/common.js
test -f tests/load/setup/seed.sql
test -f tests/load/setup/verify.sql
test -f tests/load/setup/loadtest-config.json
test -f tests/load/setup/start-loadtest.sh
test -f tests/load/setup/stop-loadtest.sh
test -f tests/load/scripts/auth/auth_service.js
test -f tests/load/scripts/user/user_service.js
test -f tests/load/scripts/photo/photo_service.js
test -f tests/load/scripts/photo/comment_service.js
test -f tests/load/scripts/photo/collection_service.js
test -f tests/load/scripts/photo/collection_photo_service.js
test -f tests/load/Makefile
test -f tests/load/config/remote.json

# 2. 检查 cleanup.sql 已删除
test ! -f tests/load/setup/cleanup.sql

# 3. 检查旧脚本已删除
test ! -f tests/load/scripts/auth.js
test ! -f tests/load/scripts/user.js
test ! -f tests/load/scripts/photo.js

# 4. 检查 BASE_URL 无默认值
! grep -q "|| 'http" tests/load/helpers/common.js

# 5. 检查 shell 脚本语法
bash -n tests/load/setup/start-loadtest.sh
bash -n tests/load/setup/stop-loadtest.sh

# 6. 检查 Makefile 语法
make -n -f tests/load/Makefile help

# 7. 检查 YAML 语法
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"

# 8. 检查 JSON 语法
jq . tests/load/setup/loadtest-config.json > /dev/null
jq . tests/load/config/remote.json > /dev/null
```

### 审查清单

- [ ] `common.js` 中 `BASE_URL` 无默认值，未设置时抛出 Error
- [ ] `seed.sql` 使用 `:'auth_users'`、`:'photo_users'`、`:'photos'` psql 变量
- [ ] `seed.sql` 中 `inviter` 字段值为 `1`（BIGINT 类型），不是 `'TEST01'`
- [ ] `cleanup.sql` 已删除
- [ ] `start-loadtest.sh` 包含 CREATE DATABASE → init.sql → seed.sql → config → start → health check 完整流程
- [ ] `stop-loadtest.sh` 包含 kill → DROP DATABASE → rm config 完整流程
- [ ] 所有 k6 脚本从 `../../helpers/common.js` 导入
- [ ] k6 脚本路由路径与源码一致（`/photo/` 带尾斜杠，`/photo/comment/{photo_id}` 等）
- [ ] Makefile 中 `loadtest` 目标使用 `trap 'make teardown' EXIT` 确保清理
- [ ] `release.yml` loadtest job 的 teardown step 使用 `if: always()`
- [ ] `remote.json` 不再包含 `baseUrl` 字段
- [ ] `loadtest-config.json` 中 Redis URL 使用 `/1` 分库

---

## 提交信息规范

```
feat(loadtest): 重构为隔离式压测架构

- 每次压测创建独立数据库 (memory_seek_loadtest) + 临时服务进程 (port 7985)
- k6 脚本按 domains 目录结构重组 (auth/user/photo)
- seed.sql 使用 psql 变量支持可配置数据量
- 删除 cleanup.sql (DROP DATABASE 替代)
- BASE_URL 无默认值，未设置时直接报错
- Makefile 支持完整隔离流程 (seed → start → k6 → teardown)
- release.yml 使用新的隔离式压测流程
```
