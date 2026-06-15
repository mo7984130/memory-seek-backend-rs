# 端到端压测方案设计

## 概述

将现有本地压测改造为端到端压测方案，直接对远程服务器进行全量 API 压测。基于 k6 + SSH 远程数据管理，集成到三分支 CI/CD 流水线。

## 目录结构

```
tests/load/
├── Makefile                    # 自动化入口
├── config/
│   └── remote.json             # 远程测试环境配置
├── scripts/
│   ├── k6.config.js            # k6 公共配置（阈值、标签）
│   ├── auth.js                 # 认证模块压测（重构）
│   ├── user.js                 # 用户模块压测（新增）
│   └── photo.js                # 照片模块压测（重构）
├── setup/
│   ├── seed.sql                # 预置测试数据
│   ├── cleanup.sql             # 压测后清理
│   └── verify.sql              # 验证数据就绪
├── helpers/
│   └── common.js               # k6 公共函数（登录、请求封装）
├── fixtures/
│   └── test.jpg                # 上传用测试图片
└── docker-compose.yml          # 保留，本地开发用
```

## 配置文件

### config/remote.json

```json
{
  "baseUrl": "https://your-server.com",
  "db": {
    "host": "localhost",
    "port": 5432,
    "database": "memory_seek_test",
    "user": "test",
    "password": "..."
  },
  "ssh": {
    "host": "your-server.com",
    "port": 22,
    "user": "deploy",
    "keyFile": "~/.ssh/deploy_key"
  }
}
```

k6 脚本通过 `BASE_URL` 环境变量切换目标地址，本地和 CI 共用脚本。

## 测试数据策略

### seed.sql

预置 1000 个 auth 测试用户 + 20 个 photo 测试用户。密码统一使用 bcrypt 哈希（明文 "Test123456"）。

- 使用 `ON CONFLICT DO NOTHING` 保证幂等
- k6 脚本通过 `__VU % 1000` 循环复用用户，不再每次迭代注册

### cleanup.sql

```sql
DELETE FROM auth_user WHERE email LIKE '%@test.com';
```

只清理压测数据，不影响其他数据。

### verify.sql

```sql
SELECT count(*) AS auth_users FROM auth_user WHERE email LIKE '%@test.com';
```

确认预置用户数量正确。

## k6 脚本设计

### 公共配置 k6.config.js

k6 不支持跨文件 ES module import，公共配置通过环境变量注入。每个脚本内部定义阈值，BASE_URL 统一从 `__ENV.BASE_URL` 读取。

```javascript
// 每个脚本内部
const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';

export const options = {
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
  },
  // ... scenarios
};
```

### 阈值标准

| 接口类型 | p(95) | 错误率 |
|---------|-------|--------|
| 认证（登录/注册） | < 200ms | < 1% |
| 用户（查询/更新） | < 200ms | < 1% |
| 照片查询 | < 200ms | < 1% |
| 照片上传 | < 1000ms | < 1% |

### auth.js

- 执行器：ramping-vus
- 加压策略：30s→50, 1m 持平, 30s→100, 1m 持平, 30s→0
- 用户来源：seed.sql 预置，`__VU % 1000` 选择
- 流程：登录 → 获取 token → 访问受保护接口

### user.js（新增）

- 执行器：ramping-vus
- 加压策略：30s→20, 1m 持平, 30s→50, 1m 持平, 30s→0
- 流程：登录 → GET /user/profile → PUT /user/profile

### photo.js

- 执行器：ramping-vus（从 shared-iterations 改为与 auth 一致）
- 用户来源：seed.sql 预置
- 流程：登录 → 上传照片 → 查询列表 → 查询详情
- 上传接口阈值放宽到 p(95)<1000ms

## 三分支 CI/CD 策略

### 分支与环境映射

| 分支 | CI（测试/lint） | CD（部署） | 压测 | Environment |
|------|----------------|-----------|------|-------------|
| `develop` | ✅ 自动 | ❌ | ❌ | — |
| `release` | ✅ 自动 | ✅ 自动 | ✅ 自动 | `loadtest` |
| `main` | ✅ 自动 | ✅ 自动 | ❌ | `production` |

### 工作流

```
develop ──CI──▶ (测试/lint/clippy，不部署)
   │
   ▼ PR 合入
release ──CI + CD + LoadTest──▶ 部署到服务器 + 压测
   │                            environment: loadtest
   ▼ PR 合入
main ──CI + CD──▶ 部署到服务器（生产）
                  environment: production
```

### Workflow 文件

#### ci.yml — 三个分支都跑

触发条件：push 和 PR 到 develop/release/main

- cargo clippy
- cargo test --lib

#### release.yml — release 分支：部署 + 压测

触发条件：push 到 release

1. build：编译 musl 静态二进制
2. deploy：SSH 上传 + systemctl restart（environment: loadtest）
3. loadtest：SSH seed → k6 × 3 → SSH cleanup（environment: loadtest）

#### deploy.yml — main 分支：仅部署

触发条件：push 到 main

1. build：编译 musl 静态二进制
2. deploy：SSH 上传 + systemctl restart（environment: production）

### GitHub Environments

| Environment | 用途 | Secrets |
|-------------|------|---------|
| `production` | 生产部署 | `SERVER_SSH_KEY`, `SERVER_HOST`, `SERVER_PORT`, `SERVER_USER` |
| `loadtest` | 压测专用 | `LOADTEST_SSH_KEY`, `LOADTEST_BASE_URL`, `LOADTEST_DB_*` |

### Branch Protection Rules

| 分支 | Required checks |
|------|----------------|
| `develop` | `test` |
| `release` | `test` + `deploy` + `loadtest` |
| `main` | `test` + `deploy` |

release 分支合入 main 时，必须部署 + 压测全部通过。

## Makefile（本地执行）

```makefile
CONFIG_FILE ?= config/remote.json
BASE_URL    ?= $(shell jq -r .baseUrl $(CONFIG_FILE))
SSH_HOST    ?= $(shell jq -r .ssh.host $(CONFIG_FILE))
SSH_PORT    ?= $(shell jq -r .ssh.port $(CONFIG_FILE))
SSH_USER    ?= $(shell jq -r .ssh.user $(CONFIG_FILE))
SSH_KEY     ?= $(shell jq -r .ssh.keyFile $(CONFIG_FILE))
DB_NAME     ?= $(shell jq -r .db.database $(CONFIG_FILE))

SSH_CMD = ssh -i $(SSH_KEY) -p $(SSH_PORT) $(SSH_USER)@$(SSH_HOST)

.PHONY: seed cleanup verify loadtest loadtest-auth loadtest-user loadtest-photo

seed:
	$(SSH_CMD) "psql -U test -d $(DB_NAME)" < setup/seed.sql

cleanup:
	$(SSH_CMD) "psql -U test -d $(DB_NAME)" < setup/cleanup.sql

verify:
	$(SSH_CMD) "psql -U test -d $(DB_NAME) -c \"SELECT count(*) FROM auth_user WHERE email LIKE '%@test.com'\""

loadtest: seed
	k6 run -e BASE_URL=$(BASE_URL) scripts/auth.js
	k6 run -e BASE_URL=$(BASE_URL) scripts/user.js
	k6 run -e BASE_URL=$(BASE_URL) scripts/photo.js
	$(MAKE) cleanup

loadtest-auth: seed
	k6 run -e BASE_URL=$(BASE_URL) scripts/auth.js
	$(MAKE) cleanup

loadtest-user: seed
	k6 run -e BASE_URL=$(BASE_URL) scripts/user.js
	$(MAKE) cleanup

loadtest-photo: seed
	k6 run -e BASE_URL=$(BASE_URL) scripts/photo.js
	$(MAKE) cleanup
```

## 失败处理

### 部署已生效但压测失败

不自动回滚。压测失败说明性能不达标，功能可能正常。由开发者决定修复或回滚。

### 失败通知

GitHub Actions 默认发送邮件通知。可配置 Slack/钉钉 webhook。

### 结果归档

k6 输出保存为 artifact，保留 30 天。

### 本地调试

```bash
make seed && make verify     # 检查数据
make loadtest-auth           # 单模块复现
make cleanup                 # 清理
```
