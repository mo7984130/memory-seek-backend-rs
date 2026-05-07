# benches - HTTP 压力测试工具

自定义的异步 HTTP 压力测试工具，用于对 memory-seek 后端的认证接口进行负载测试。

## 架构概览

```
benches/
├── Cargo.toml              # crate 配置，包含 stress 二进制和 criterion bench
├── config/
│   └── bench.toml          # 测试配置（目标地址、并发数、场景权重等）
├── data/
│   └── password_hashes.txt # 预生成的 100 个 argon2id 密码哈希
├── benches/
│   └── stress.rs           # criterion bench 占位文件（暂未使用）
└── src/
    ├── main.rs             # CLI 入口，解析参数，构建场景，启动 Runner
    ├── lib.rs              # 模块导出
    ├── config.rs           # 配置加载与用户分配算法
    ├── runner.rs           # 并发调度核心（Semaphore 控制并发，加权随机选择场景）
    ├── metrics.rs          # 延迟指标采集（HdrHistogram，记录 p50/p90/p99/max/min/mean）
    ├── reporter.rs         # 终端报告输出（表格形式）
    ├── seed.rs             # 测试数据初始化（写入 DB 用户、Redis 邀请码/验证码）
    └── scenarios/
        ├── mod.rs           # Scenario trait 定义 + WeightedScenario 结构
        └── auth/
            ├── mod.rs
            ├── login.rs        # 登录场景
            ├── register.rs     # 注册场景
            └── token_refresh.rs # Token 刷新场景
```

## 核心原理

### 并发模型

- 使用 **Tokio 异步运行时** + **Semaphore** 控制并发上限
- 在 `duration_secs` 时间窗口内不断获取信号量、随机选择场景、spawn 异步任务
- 场景选择采用 **加权随机**：将场景按 weight 展开为扁平列表，每次 `random_range(0..len)` 选取

### 用户分配

配置中 `needs_credentials = true` 的场景按权重比例瓜分用户池。例如 100 个用户、login weight=60、refresh weight=30，则 login 分配 67 个、refresh 分配 33 个。注册场景不需要预分配用户。

### 指标采集

- 使用 **HdrHistogram**（精度 3 位有效数字）记录每次请求的延迟（微秒）
- 线程安全：`Mutex<Histogram>` + `AtomicU64` 计数器
- 最终输出 p50 / p90 / p99 / max / min / mean / 总请求数 / 成功数 / 错误数 / 错误率

### Token 管理

- `TokenStore` 在 warmup 阶段批量登录所有测试用户，缓存 access_token / refresh_token
- `get_auth()` 在 token 即将过期时自动刷新（double-check locking）
- Token 刷新场景直接调用 `TokenStore::get_auth()`，走内部刷新逻辑

## 使用方法

### 前置条件

1. 启动 memory-seek 后端服务（默认 `http://localhost:3000`）
2. 准备测试数据库（名称须包含 `test`，安全检查）
3. 准备 Redis 实例
4. 在项目根目录创建 `.env.test` 文件设置环境变量（**注意：seed 子命令会加载 `.env.test` 而非 `.env`，避免误操作生产数据库**）：

```bash
DATABASE_URL=postgres://user:pass@localhost:5432/memory_seek_test
REDIS_URL=redis://127.0.0.1:6379/1
```

### Step 1: 初始化测试数据

```bash
# 一键初始化（写入 100 个测试用户 + Redis 邀请码 + 邮箱验证码）
cargo run -p benches --bin stress -- seed

# 或分步执行：
cargo run -p benches --bin stress -- insert-test-users     # 写入 testuser0..99
cargo run -p benches --bin stress -- set-inviter-code       # 设置邀请码
cargo run -p benches --bin stress -- set-email-codes        # 批量设置邮箱验证码
```

测试用户：`testuser0` ~ `testuser99`，密码均为 `123456abc`

### Step 2: 运行压力测试

```bash
# 使用默认配置
cargo run -p benches --bin stress

# 指定配置文件
cargo run -p benches --bin stress -- -c benches/config/bench.toml

# 覆盖并发数和持续时间
cargo run -p benches --bin stress -- -n 100 -d 30
```

### 阶梯式加压测试

```bash
# 使用阶梯式模式（配置文件中 mode = "staircase"）
cargo run -p benches --bin stress

# 固定并发模式（配置文件中 mode = "fixed"）
cargo run -p benches --bin stress

# 导出结果
cargo run -p benches --bin stress -- --export json,csv

# 指定导出目录
cargo run -p benches --bin stress -- --export json --output-dir ./my_results
```

### 命令行参数

| 参数 | 简写 | 说明 | 默认值 |
|------|------|------|--------|
| `--config` | `-c` | 配置文件路径 | `benches/config/bench.toml` |
| `--concurrency` | `-n` | 并发数（覆盖配置） | 配置文件中的值 |
| `--duration` | `-d` | 持续时间（秒，覆盖配置） | 配置文件中的值 |

### 子命令

| 命令 | 说明 |
|------|------|
| `seed` | 一键初始化所有测试数据 |
| `insert-test-users` | 写入 100 个测试用户到 auth_user 表 |
| `set-inviter-code` | 设置邀请码到 Redis（默认 user_id=1） |
| `set-email-codes` | 批量设置邮箱验证码到 Redis（1000 个） |

## 配置说明 (`bench.toml`)

```toml
[server]
base_url = "http://localhost:3000"    # 目标服务地址

[auth]
refresh_before_expiry_secs = 60       # token 提前刷新阈值（秒）

# 用户定义 - 方式1：逐个指定
[[users]]
account = "testuser0"
password = "123456abc"

# 用户定义 - 方式2：模式展开（testuser0..testuser99）
[[user_patterns]]
pattern = "testuser{0..100}"
password = "123456abc"

[bench]
concurrency = 50                      # 最大并发数
duration_secs = 60                    # 测试持续时间（秒）
warmup_secs = 5                       # 预热时间（秒），不计入指标

# 场景配置（按权重分配请求比例）
[[scenarios]]
name = "auth/login"
weight = 60                           # 60% 的请求
needs_credentials = true              # 需要预分配用户凭证

[[scenarios]]
name = "auth/token_refresh"
weight = 30                           # 30% 的请求
needs_credentials = true

[[scenarios]]
name = "auth/register"
weight = 10                           # 10% 的请求
# needs_credentials 默认 false，不需要预分配
```

## 场景说明

### auth/login

- 随机选取已分配的测试用户，发送 POST `/auth/login`
- 请求体：`{ account, password }`
- 测量登录接口的响应延迟

### auth/register

- 使用原子计数器生成唯一用户名 `bench_user_{hex}`
- 发送 POST `/auth/register`，携带预设的邀请码和邮箱验证码
- 请求体：`{ username, email, password, nickname, inviter_code, email_verify_code }`

### auth/token_refresh

- 随机选取用户 ID，调用 `TokenStore::get_auth()` 触发 token 刷新
- 测量 token 刷新（含可能的 HTTP 刷新请求）的延迟

## 输出示例

```
┌─────────────────────────────────────────────────┐
│ Scenario: aggregate                             │
├─────────────────────────────────────────────────┤
│ Total requests: 12345                           │
│ Success: 12300                                  │
│ Errors: 45                                      │
│ Error rate: 0.36%                               │
├─────────────────────────────────────────────────┤
│ Latency (p50):  12.34ms                         │
│ Latency (p90):  45.67ms                         │
│ Latency (p99):  123.45ms                        │
│ Latency (max):  456.78ms                        │
│ Latency (min):  2.10ms                          │
│ Latency (mean): 18.92ms                         │
└─────────────────────────────────────────────────┘
```

## 扩展新场景

1. 在 `src/scenarios/auth/` 下创建新文件（或新建 `src/scenarios/xxx/` 目录）
2. 实现 `Scenario` trait：

```rust
use async_trait::async_trait;
use crate::scenarios::Scenario;
use crate::metrics::MetricsRecorder;
use ::auth::client::AuthClient;

pub struct MyScenario { /* fields */ }

#[async_trait]
impl Scenario for MyScenario {
    fn name(&self) -> &str { "my/scenario" }

    async fn execute(&self, client: &AuthClient, recorder: &MetricsRecorder) -> anyhow::Result<()> {
        let start = std::time::Instant::now();
        // ... 发送请求 ...
        recorder.record(start.elapsed(), success);
        Ok(())
    }
}
```

3. 在 `main.rs` 的 `match sw.name.as_str()` 中注册新场景
4. 在 `bench.toml` 的 `[[scenarios]]` 中添加配置

## 注意事项

- **环境变量**：seed 子命令自动加载 `.env.test`（`dotenvy::from_filename_override`），主压测流程加载 `.env`。两者隔离，避免误连生产数据库
- **安全检查**：`seed` 命令会检查数据库名是否包含 `test`，防止误操作生产数据库
- **密码哈希**：`data/password_hashes.txt` 包含 100 个预生成的 argon2id 哈希，对应密码 `123456abc`
- **criterion bench**：`benches/stress.rs` 当前为占位文件，实际压测通过 `cargo run -p benches --bin stress` 运行
- **内存**：HdrHistogram 内存占用固定（~20KB），不受请求量影响
