# 测试架构设计文档

## 概述

为 memory-seek-backend-rs 服务设计全链路集成测试架构，包括功能测试和性能测试，提供完整的压测数据（响应时间分布、吞吐量、资源监控、并发安全）。

## 技术栈

- **测试框架**: Rust 集成测试 + k6 负载测试
- **外部依赖**: PostgreSQL、Redis（真实服务）、MinIO（S3 兼容）
- **环境管理**: Docker Compose 全自动管理
- **CI/CD**: GitHub Actions

## 架构设计

### 目录结构

```
memory-seek-backend-rs-new/
├── tests/
│   ├── integration/              # Rust 集成测试
│   │   ├── mod.rs               # 共享测试工具
│   │   ├── helpers/             # 测试辅助函数
│   │   │   ├── mod.rs
│   │   │   ├── app.rs          # 构建测试用 AppState
│   │   │   ├── auth.rs         # 认证辅助（获取 token）
│   │   │   ├── db.rs           # 数据库清理/种子数据
│   │   │   └── minio.rs        # MinIO bucket 管理
│   │   ├── auth/               # 认证模块测试
│   │   │   ├── login.rs
│   │   │   ├── register.rs
│   │   │   └── token.rs
│   │   ├── photo/              # 图片模块测试
│   │   │   ├── upload.rs
│   │   │   ├── query.rs
│   │   │   └── delete.rs
│   │   └── user/               # 用户模块测试
│   │       └── profile.rs
│   └── load/                   # k6 负载测试
│       ├── scripts/
│       │   ├── auth.js
│       │   ├── photo.js
│       │   └── scenarios.js
│       ├── fixtures/           # 测试数据（图片等）
│       ├── results/            # 测试结果输出
│       ├── docker-compose.yml  # 测试环境
│       └── Makefile            # 快捷命令
├── Cargo.toml                  # 添加测试依赖
└── ...
```

### Docker Compose 测试环境

```yaml
# tests/load/docker-compose.yml
version: '3.8'

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: memory_seek_test
      POSTGRES_USER: test
      POSTGRES_PASSWORD: test
    ports:
      - "5433:5432"
    volumes:
      - ../../docs/sql/init.sql:/docker-entrypoint-initdb.d/init.sql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U test"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    ports:
      - "6380:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 5s
      retries: 5

  minio:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: minioadmin
      MINIO_ROOT_PASSWORD: minioadmin
    ports:
      - "9000:9000"
      - "9001:9001"
    volumes:
      - minio_data:/data
    healthcheck:
      test: ["CMD", "mc", "ready", "local"]
      interval: 5s
      timeout: 5s
      retries: 5

  app:
    build:
      context: ../..
      dockerfile: server/Dockerfile
    ports:
      - "3000:3000"
    environment:
      MEMORY_SEEK_CONFIG_PATH: /app/config.test.json
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
      minio:
        condition: service_healthy
    profiles:
      - load

volumes:
  minio_data:
```

**设计要点：**
- PostgreSQL 使用 5433 端口，Redis 使用 6380 端口，避免与本地开发环境冲突
- 使用 healthcheck 确保依赖服务就绪
- 应用服务放在 `load` profile 下，功能测试时不需要启动
- init.sql 自动初始化测试数据库 schema

### Rust 集成测试

#### 共享测试工具

```rust
// tests/integration/helpers/app.rs
use std::sync::Arc;
use sea_orm::DatabaseConnection;

/// 构建测试用 AppState
pub async fn build_test_state() -> (AppState, impl Drop) {
    let config = AppConfig::from_json("config.test.json");
    let bases = AppBase::init(&config).await;
    let libs = AppLibs::init(&config);
    let state = AppState::from(bases, libs);
    (state, CleanupGuard::new(state.db.clone()))
}

/// 构建带 MinIO 的测试 AppState
pub async fn build_test_state_with_minio(minio_uri: &str) -> (AppState, impl Drop) {
    // 类似 build_test_state，但覆盖 S3 配置
}
```

```rust
// tests/integration/helpers/auth.rs
/// 获取测试用 access_token
pub async fn get_test_token(client: &TestClient) -> String {
    // 注册 -> 登录 -> 返回 token
}
```

```rust
// tests/integration/helpers/db.rs
/// 测试数据清理 guard
pub struct CleanupGuard {
    db: DatabaseConnection,
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        // 清理测试数据
    }
}
```

```rust
// tests/integration/helpers/minio.rs
use s3::Bucket;

pub async fn setup_test_bucket(minio_uri: &str) -> Bucket {
    let bucket = Bucket::new(
        "test-bucket",
        s3::Region::Custom {
            region: "us-east-1".to_string(),
            endpoint: minio_uri.to_string(),
        },
        s3::creds::Credentials::new(
            Some("minioadmin"),
            Some("minioadmin"),
            None, None, None,
        ).unwrap(),
    ).unwrap();

    bucket.put_bucket_with_configuration(/* ... */).await.ok();
    bucket
}
```

#### 测试示例

```rust
// tests/integration/auth/login.rs
#[tokio::test]
async fn test_login_success() {
    let (state, _guard) = build_test_state().await;
    let client = TestClient::new(app(state));

    let resp = client.post("/login")
        .json(&json!({
            "account": "test@example.com",
            "password": "Test123456"
        }))
        .await;

    assert_eq!(resp.status(), 200);
    let body: R<UserDTO> = resp.json().await;
    assert!(body.data.access_token.is_some());
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (state, _guard) = build_test_state().await;
    let client = TestClient::new(app(state));

    let resp = client.post("/login")
        .json(&json!({
            "account": "test@example.com",
            "password": "wrong_password"
        }))
        .await;

    assert_eq!(resp.status(), 400);
}
```

**关键设计：**
- `CleanupGuard` 实现 `Drop`，测试结束自动清理数据
- `build_test_state()` 统一构建测试环境
- `get_test_token()` 复用认证流程，简化需要登录的测试
- 每个测试独立，不依赖执行顺序

### k6 负载测试

#### 认证测试

```javascript
// tests/load/scripts/auth.js
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

const loginErrorRate = new Rate('login_errors');
const loginDuration = new Trend('login_duration');

export const options = {
  stages: [
    { duration: '30s', target: 50 },
    { duration: '1m', target: 50 },
    { duration: '30s', target: 100 },
    { duration: '1m', target: 100 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],
    login_errors: ['rate<0.1'],
  },
};

const BASE_URL = __ENV.APP_URL || 'http://localhost:3000';

// 用户池隔离：每个 VU*1000+ITER 生成唯一用户
function getUserCredentials(vuId, iterId) {
  const userId = vuId * 1000 + iterId;
  return {
    account: `loadtest_user_${userId}@test.com`,
    password: 'Test123456',
  };
}

// 预注册用户
export function setup() {
  const totalUsers = 100 * 1000;
  const registered = [];

  for (let i = 0; i < totalUsers; i++) {
    const { account, password } = getUserCredentials(0, i);
    const res = http.post(`${BASE_URL}/register`, JSON.stringify({
      username: `loadtest_${i}`,
      email: account,
      password: password,
      nickname: `Test User ${i}`,
    }), {
      headers: { 'Content-Type': 'application/json' },
    });

    if (res.status === 200) {
      registered.push(account);
    }
  }

  return { registeredCount: registered.length };
}

export default function (data) {
  const { account, password } = getUserCredentials(__VU, __ITER);

  const loginRes = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password,
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  check(loginRes, {
    'login status is 200': (r) => r.status === 200,
    'login has token': (r) => r.json('data.access_token') !== undefined,
  });

  loginErrorRate.add(loginRes.status !== 200);
  loginDuration.add(loginRes.timings.duration);

  if (loginRes.status !== 200) {
    console.error(`Login failed for ${account}: ${loginRes.body}`);
    return;
  }

  sleep(1);
}
```

#### 图片测试

```javascript
// tests/load/scripts/photo.js
import http from 'k6/http';
import { check } from 'k6';
import { SharedArray } from 'k6/data';

const testImages = new SharedArray('images', function () {
  return [
    open('./fixtures/test.jpg', 'b'),
    open('./fixtures/test.png', 'b'),
  ];
});

export const options = {
  scenarios: {
    upload: {
      executor: 'shared-iterations',
      vus: 20,
      iterations: 200,
      maxDuration: '5m',
    },
  },
};

const tokens = {};

function getToken() {
  if (tokens[__VU]) {
    return tokens[__VU];
  }

  const account = `loadtest_photo_${__VU}@test.com`;
  const loginRes = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password: 'Test123456',
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  if (loginRes.status === 200) {
    tokens[__VU] = loginRes.json('data.access_token');
  }

  return tokens[__VU];
}

export default function () {
  const token = getToken();
  if (!token) return;

  const headers = { Authorization: `Bearer ${token}` };

  const image = testImages[__VU % testImages.length];
  const uploadRes = http.post(`${BASE_URL}/photo`, {
    file: http.file(image, 'test.jpg', 'image/jpeg'),
  }, { headers });

  check(uploadRes, {
    'upload success': (r) => r.status === 200,
  });

  const queryRes = http.get(`${BASE_URL}/photo?limit=20`, { headers });
  check(queryRes, {
    'query success': (r) => r.status === 200,
  });
}
```

**关键设计：**
- `getUserCredentials(VU, ITER)` 生成全局唯一的用户标识，确保用户池隔离
- `setup()` 阶段批量注册测试用户
- 每个 VU 维护独立的 token，避免登录后旧 token 失效导致的冲突
- `thresholds` 定义性能基线（P95 < 500ms，错误率 < 10%）
- `SharedArray` 共享测试数据，减少内存占用

### Makefile 快捷命令

```makefile
# tests/load/Makefile

.PHONY: test-unit test-integration test-load test-all clean

test-unit:
	cargo test --lib

test-integration:
	docker compose up -d postgres redis minio
	sleep 10
	cargo test -p server --features "auth,user,photo" -- --test-threads=1
	docker compose down -v

test-load: test-load-auth test-load-photo

test-load-auth:
	docker compose --profile load up -d
	sleep 30
	k6 run scripts/auth.js --out json=results/auth.json
	docker compose --profile load down -v

test-load-photo:
	docker compose --profile load up -d
	sleep 30
	k6 run scripts/photo.js --out json=results/photo.json
	docker compose --profile load down -v

test-all: test-unit test-integration test-load

clean:
	docker compose --profile load down -v
	rm -rf results/*.json results/*.csv
```

### CI/CD 集成

```yaml
# .github/workflows/test.yml
name: Test & Benchmark

on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]

jobs:
  integration-test:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache dependencies
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Start test services
        run: |
          docker compose -f tests/load/docker-compose.yml up -d postgres redis minio
          sleep 10
          docker compose -f tests/load/docker-compose.yml exec -T postgres pg_isready -U test
          docker compose -f tests/load/docker-compose.yml exec -T redis redis-cli ping

      - name: Run integration tests
        run: cargo test -p server --features "auth,user,photo" -- --test-threads=1
        env:
          DATABASE_URL: postgres://test:test@localhost:5433/memory_seek_test
          REDIS_URL: redis://localhost:6380
          MINIO_ENDPOINT: http://localhost:9000

      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: test-results
          path: target/test-results/

  load-test:
    runs-on: ubuntu-latest
    needs: integration-test
    if: github.ref == 'refs/heads/main' || github.ref == 'refs/heads/master'

    steps:
      - uses: actions/checkout@v4

      - name: Install k6
        run: |
          sudo gpg -k
          sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D68
          echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
          sudo apt-get update
          sudo apt-get install k6

      - name: Start full environment
        run: |
          docker compose -f tests/load/docker-compose.yml --profile load up -d
          sleep 30

      - name: Run load tests
        run: |
          cd tests/load
          k6 run scripts/auth.js --out json=results/auth.json
          k6 run scripts/photo.js --out json=results/photo.json

      - name: Upload load test results
        uses: actions/upload-artifact@v4
        with:
          name: load-test-results
          path: tests/load/results/

      - name: Cleanup
        if: always()
        run: docker compose -f tests/load/docker-compose.yml --profile load down -v
```

**CI 设计要点：**
- 两阶段执行：先跑集成测试，通过后再跑负载测试
- 主分支才跑负载测试，避免 PR 浪费资源
- 缓存依赖加速构建
- 自动清理容器和数据卷
- 测试结果作为 artifact 保存

## 测试覆盖范围

### 功能测试

| 模块 | 测试场景 |
|------|---------|
| Auth | 登录成功/失败、注册成功/失败、Token 刷新、邮箱验证 |
| Photo | 上传成功/失败、分页查询、MD5 查重、图片删除、图片访问 |
| User | 用户信息查询、更新 |

### 性能测试

| 指标 | 说明 |
|------|------|
| 响应时间分布 | P50/P95/P99 延迟 |
| 吞吐量 | QPS（每秒请求数） |
| 资源监控 | 内存、CPU、连接数 |
| 并发安全 | 高并发下的数据一致性 |

## 配置文件

### config.test.json

```json
{
  "database_config": {
    "url": "postgres://test:test@localhost:5433/memory_seek_test",
    "max_connections": 10
  },
  "redis_config": {
    "url": "redis://localhost:6380"
  },
  "oss_config": {
    "bucket": "test-bucket",
    "region": "us-east-1",
    "endpoint": "http://localhost:9000",
    "access_key": "minioadmin",
    "secret_key": "minioadmin"
  },
  "token_cipher_config": {
    "key": "test-secret-key-for-testing-only"
  }
}
```

## 依赖更新

### Cargo.toml 添加

```toml
[dev-dependencies]
reqwest = { version = "0.12", features = ["json", "multipart"] }
tokio-test = "0.4"
wiremock = "0.6"
serde_json = "1.0"
```

## 执行流程

### 本地开发

```bash
# 1. 运行单元测试
make test-unit

# 2. 运行集成测试
make test-integration

# 3. 运行负载测试
make test-load

# 4. 运行所有测试
make test-all
```

### CI/CD

1. PR 提交 → 触发集成测试
2. 合并到 main → 触发集成测试 + 负载测试
3. 测试结果上传为 artifact
4. 可选：集成 Grafana 可视化报告

## 注意事项

1. **测试隔离**：每个测试使用独立数据，通过 CleanupGuard 自动清理
2. **用户池隔离**：k6 中每个虚拟用户使用独立账号，避免 token 冲突
3. **端口冲突**：Docker 服务使用非标准端口（5433/6380/9000）
4. **CI 资源**：负载测试仅在主分支运行，节省 CI 资源
5. **测试数据**：fixtures 目录存放测试图片等静态资源
