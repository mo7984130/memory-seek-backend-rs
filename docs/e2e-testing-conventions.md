# E2E 测试约定

`tests/e2e`(Rust) 以**功能正确性**为目标:真实 HTTP 调用 + 副作用回查(DB / Redis / MailHog)。
性能与容量归 `tests/load`(k6),两者不混用。

配套文档:`docs/service-conventions.md`(埋点)、`tests/monitoring/dashboards/metrics-naming.md`(指标命名)。

## 目录与运行

```
tests/e2e/src/
├── main.rs      # 组装 Context、灌种子、运行全部场景
├── context.rs   # HTTP client + DB + MailHog + Redis + S3
├── config.rs    # 配置: CLI > E2E_CONFIG_PATH > tests/e2e/e2e.config.yml
├── preprea.rs   # 种子数据(先清空再灌入, 可重复执行)
└── scenarios/{module}/{endpoint}.rs   # 一个文件 = 一个端点的全部场景(正/负同文件)
```

```sh
docker compose -f tests/docker-compose.yml up -d --wait
# server 需完整 features(photo 表依赖 face-engine); CWD=tests 以解析模型相对路径
( cd tests && ../target/debug/memory-seek-server --config config.yml ) &
cargo run -p e2e -- -c tests/e2e/e2e.config.yml
```

## 场景定义

`Scenario`(memseek-test)固定四段:`type Ctx / Error / Output / Setup` + `setup` / `run` / `validate`。

```rust
#[derive(Default)]
pub struct LoginScenario;

impl Scenario for LoginScenario {
    type Ctx = Context;
    type Error = HttpError;
    type Output = SucR<LoginResponse>; // run 的产出, 供 validate 使用
    type Setup = Session;              // 每任务一次的前置产出; 不需要时填 ()

    async fn setup(ctx: &Self::Ctx, task: &TaskIndex) -> Result<Self::Setup, Self::Error> {
        login(ctx, &account(task.index)).await // 登录一次, 供该任务各轮复用
    }
    async fn run(ctx: &Self::Ctx, task: &TaskIndex, setup: &Self::Setup)
        -> Result<Self::Output, Self::Error> { /* 只发请求, 不校验 */ }
    async fn validate(ctx: &Self::Ctx, task: &TaskIndex, setup: &Self::Setup, output: &Self::Output)
        -> Result<bool, Self::Error> { /* 只断言副作用: DB / Redis / MailHog / S3 */ }
}

register_scenario!(LoginScenario, mode = memseek_test::RunMode::Times(32));
```

- 命名:`{端点}Scenario`;负例加语义后缀(`LoginWrongPasswordScenario`)。
- `run` 返回 `Err` 该轮直接失败、不进 `validate`;`validate` 返回 `Ok(false)` 记一次 validate 失败。
- `Setup` 必须 `Default + Send + Sync`;含强类型 ID(未实现 `Default`)时用 `i64` 存或手写 `Default`。

## 运行模式与并发

`ManagerConfig::new(concurrency)`(当前 32)全局控制并发;`register_scenario!(mode = ...)` 声明默认模式。

- **功能验证一律 `Times(n)`,不用 `Duration`**;`n` 是**全局总次数**,按并发分片,
  `task.index ∈ 0..concurrency` 是**任务号而非轮次**(同任务各轮 index 相同)。
- 写场景(注册/发码/上传等有副作用)必须"每次 run 数据唯一":用 `task.index` 造唯一账号/内容,
  且 `n ≤ concurrency`,保证每任务恰好 1 次(避免重复注册等冲突)。
- 读场景(登录/列表)可用任意 `n`。

## 断言

统一响应(`common/src/axum/r.rs`):成功 `200 + {code,data}` → `SucR<T>`;业务错误 `4xx + {code,msg}` → `ErrR`。

- **正向**:`client.get/post/...`(非 2xx 直接 Err),或 builder 的 `send_checked()`。
- **负向**:必须用 `*_raw`(`post_raw` / `get_raw` / `send()`)拿原始响应,自行断言 `output.code == 400/401`。
- `validate` 覆盖副作用:落库一致、状态变化、防重放(如注册后验证码已删);
  响应不可观测时(如发信)用 MailHog 查邮件作为副作用断言。

## 数据与清理

- 复用的种子账号**只读**;写场景需唯一内容,并在 `preprea` 清理(保证可重复运行)。
- `preprea::init` 每次先清空(含重置序列)再灌入。schema sync **只增不改**,
  老库列/默认值变更需手工 `ALTER`(见 `docs/service-conventions.md` 的 schema 小节)。
- 账号池(密码 `Test123456`):`loadtest_{1..N}`(auth)、`loadtest_photo_{1..N}`(photo,
  每号 20 张 `seed_file_{u}_{p}`)、`uit_user_*` / `uit_pwd_*`(user,改密正例独立池)。
- MailHog:`Content.Body` 是 MIME base64(解码前去空白);按收件人过滤,写场景用独立邮箱命名空间。

## 覆盖范围

| 模块 | 状态 |
|---|---|
| auth / user / photo / system(`GET /metrics`) | 已覆盖 |
| backup / audit、admin 端点 | 暂不覆盖 |

photo 模块要点(见 `scenarios/photo/`):

- **上传内容必须唯一**(`photo_photo.md5` 全局唯一):`unique_png()` 在 PNG 尾部追加唯一标记;
  fixture 必须是**能真正解码**的 PNG —— `FileValidator` 只读文件头/尺寸,而 face 管线会完整解码。
- 列表/详情是**全局视图**(不按 `user_id` 过滤),断言勿假定记录归属当前用户。
- 列表要绕过 24h 首屏缓存时,显式传 `direction` + `cursor` 走 keyset 分支。

## 新增模块/场景检查清单

- [ ] 一个文件一个端点,正/负同文件,`mod.rs` 只声明。
- [ ] `Ctx / Error / Output / Setup` 正确,`Setup` 满足 `Default`。
- [ ] 功能验证 `Times(n)`;写场景 `n ≤ concurrency` 且数据唯一。
- [ ] 负向用 `*_raw` 自行断言 status/`code`;`validate` 覆盖副作用。
- [ ] 自建数据(含上传的对象存储对象)已在 `preprea` 清理。

## 踩坑记录

- **指标名保留冒号**:Prometheus exporter 只把非法字符(如 `.`)替换为 `_`,而 `:` 合法会保留,
  故领域指标是 `auth:login:attempts` 而非 `auth_login_attempts`;counter 另带 `_total` 后缀。
- **删照片的 S3 对象是异步删除**(事件消费者,最终一致):断言需带超时轮询等待。
- **`server.build_info` 的 commit** 由 `build.rs` 注入(依赖 `GITHUB_SHA` 与 git ref 变化触发重建)。
- server 需完整 features 构建;写场景 `Times(n)` 的 `n ≤ 32`。
