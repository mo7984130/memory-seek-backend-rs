# memory-seek-backend 开发约定

## 仓库结构

- `types/*`：SeaORM 实体、DTO 与跨上下文契约；实体仅在 `orm` feature 下注册。
- `domains/*`：业务域（`repo` / `services` / `controllers`）；`auth` / `user` / `visual` 的 controller 受 `controller` feature 控制。
- `common/*`：错误、数据库、Redis、Web、指标等基础设施；`libs/*`：邮件、S3、校验、宏等通用库。
- `server`：组合根，只做配置加载、依赖装配、中间件与启动。
- `clients/api-rs`：Rust API 客户端，DTO 直接复用 `types-*`；`tests/e2e`：真实栈端到端测试。
- `deploy/ansible`：生产部署（systemd + PostgreSQL/Redis/Nginx），见该目录 README。
- `build/`：构建与打包脚本；`tools/`：仓库工具（版本号、运行时资产 hash 等）。

依赖方向：`types/*` 不依赖 `domains/*`；`domains/*` 依赖 `types/*` 与 `common/*`；`server` 依赖所有业务域。

## 构建与验证

本地一律使用 dev profile（Cargo 默认 profile）；CI 仍用 `--release`（见 `.github/workflows/ci.yml`），本地验收不要求。

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib                       # workspace 单元测试
cargo test -p visual <test_name>       # 单个测试
cargo test -p memory-seek-api-rs       # 客户端集成测试
```

- 根是 virtual workspace 且未设 `default-members`：构建 server 时必须带 `-p server`，否则会连 `tests/e2e` 一起编译。
- `.cargo/config.toml` 固定 `-fuse-ld=mold`、`target-cpu=x86-64-v3`；Linux x86_64 缺 mold 会链接失败，不要往 `[build]` 里另加 rustflags。
- `server/build.rs` 无条件链接 `thirdparty/onnxruntime/libonnxruntime.so`，编译前需 `git lfs pull`。运行 `cargo run -p server` 时需 `LD_LIBRARY_PATH=thirdparty/onnxruntime`（或把 `.so` 放到 `<target>/<profile>/libs`）。
- `server` 默认 feature 为空，裸 `cargo build -p server` 不含任何业务域。完整构建：

```bash
cargo build -p server --features "metrics,auth,user,visual,face-engine,audit,audit-recording,backup,admin"
```

## 配置与启动

- 配置查找顺序：`--config` / `-c` > `MEMORY_SEEK_CONFIG_PATH` > `./config.yml` > `/etc/memory-seek-server/config.yml`；从 `example.config.yaml` 复制起步。
- 环境变量覆盖格式为 `MEMORY_SEEK_<SECTION>__<KEY>`（双下划线），如 `MEMORY_SEEK_SERVER__PORT=8080`、`MEMORY_SEEK_DATABASE__URL=...`。
- `AppConfig` 的字段按 feature 条件编译：开启某个域/能力后，对应配置段变成必填（如 `smtp`、`s3`、`token_cipher`、`face_engine`）。
- `visual` 启动时会校验 ffmpeg 并探测 `libsvtav1`，缺失直接启动失败；容器部署依赖 `build/Dockerfile.runtime` 内置的 ffmpeg，裸机部署由 ansible 角色安装。
- 新增业务域需同时补 `server/Cargo.toml` feature、`server/src/config/mod.rs` 的 cfg 字段、`server/src/setup/domains/` 的模块与 `#[cfg(feature = ...)]` 声明。

## 打包与部署

- 产物按变更频率分层：二进制包(≈13MB, 每次发版)与运行时资产包 libs+models(≈180MB, 按内容 hash 发布)。`build/package-dist.sh` 的 `--mode=release|dist` 分别产出两者，本地 compose 与 CI 发布共用。
- 二进制包内 `manifest.yaml` 声明依赖的运行时资产 hash；改 `thirdparty/**` 后由 `Runtime Assets` workflow 自动发布新资产包，不需要手工改版本号。
- 生产部署走 `deploy/ansible`（`cd deploy/ansible && ansible-playbook server.yml`）。部署版本固定写在 `deploy/ansible/group_vars/all/versions.yml`（不读本地仓库状态），回滚用 `-e binary_version=vX.Y.Z`；CI 校验它与 `server/Cargo.toml` 一致，bump 版本时必须同步修改。
- 部署相关校验：`cd deploy/ansible && ansible-lint && ansible-playbook --syntax-check site.yml`。

## 启动编排与路由

- 各初始化步骤用 `#[common_macros::register_async(slice = ..., ty = crate::setup::InitFn)]` 自注册到 linkme 切片，不按源码顺序执行；依赖通过 `AppSetup.registry`（`TypeMap`）传递，缺失依赖用 `MissDepError` 报错。
- `admin` 不走统一切片，在 `AppSetup::init` 最后手动挂载，因为它依赖 user/visual/backup 的 state。
- 路由分 `public` / `protected`；受保护 handler 从 `Extension<UserId>` 取登录用户，返回值用 `R<T>` + `to_r_ok()`。
- server 自身不挂 `/api` 前缀（实际为 `/auth`、`/user`、`/visual`、`/admin`）；`clients/api-rs` 默认 `base_url` 是 `/api`，用于反代/同源部署，直连时需显式传根地址（e2e 即如此）。
- 改接口后同步更新 `clients/api-rs` 及其 `tests/`，以及 `tests/e2e/src/scenarios/`，否则契约会漂移。

## 数据库

- 没有 migration 文件。启动时 `types-db-init::init_db` 按 SeaORM 实体注册表 `sync` 建表，只会新增，不会删除/修改已有列。
- 新实体 crate 必须依赖 `types-db-registry` 并登记 `SCHEMA_PREFIXES`；否则表不会被创建。索引用 `#[common_macros::register_async(slice = types_db_registry::INIT_INDEXES, ...)]` 注册为幂等回调。
- 删列、改列、删表或修正 NOT NULL/默认值必须写手工 SQL；已存在表不会因实体变更自动对齐。
- admin 多实例任务模型见 `domains/admin/README.md`（DB 为唯一事实源，`run_id` 做 fencing）。

## 测试环境

- e2e 需要 PostgreSQL+pgvector、Valkey、MailHog、rustfs，以及一个独立启动的 server（默认 `127.0.0.1:7985`）；`cargo run -p e2e` 不会自动拉起 server。
- 启动依赖栈（本机用 podman；CI 用 docker）：

```bash
podman compose -f tests/run/docker-compose.yml down -v
podman compose -f tests/run/docker-compose.yml up -d --wait postgres valkey mailhog rustfs
podman compose -f tests/run/docker-compose.yml run --rm postgres-init
podman compose -f tests/run/docker-compose.yml run --rm rustfs-init
```

- 本地完整环境：`./build/build.sh` 生成 `target/dist`，`./build/runtime-image.sh` 构建运行镜像，再 `podman compose -f tests/run/docker-compose.yml up -d server`。
- e2e 默认 `run.prepare=true`，会先清空并重灌种子数据库；不要指向共享库，复用数据时设 `run.prepare=false`。
- 常用覆盖：`E2E__RUN__SCENARIOS=UploadVisualScenario` 只跑单场景（名字是注册的 Rust 类型名，不是文件路径），`E2E__RUN__CONCURRENCY` 不得超过种子池大小（`auth_users` / `visual_users` / `uit_users`）。
- 常用命令：`cargo run -p e2e -- -c tests/e2e/e2e.config.yml`。

## 版本与发版

- 全局版本只有一处：`server/Cargo.toml` 的 `version`。启动日志与 `--version`
  都取自它（`env!("CARGO_PKG_VERSION")`），其余子 crate 保留各自版本，不参与对外版本。
- **无论修改哪个子 crate，都必须在同一个 PR 里递增 `server` 的版本**：

  ```bash
  tools/bump-version.sh 2.7.0
  ```

  该脚本只改 `server/Cargo.toml` 并刷新 `Cargo.lock`。漏改的表现是 `main` 上
  找不到新版本 tag，发布被跳过（CI 会打印跳过原因）。
- `main` 的 push 触发 `.github/workflows/release.yml`：读取 `server` 版本，
  tag `v<version>` 不存在时构建、打 tag、用 GitHub 自动 notes 发布；已存在则跳过。
- 版本由人决定，不由提交类型自动递增；不依赖任何自动版本工具。
- 提交信息仍使用 Conventional Commits，仅用于 PR 标题风格与 release notes 可读性。

## 分支与提交

- CI（fmt / clippy / 单元测试 / e2e）只跑 `develop` 的 push 与 PR。

## 错误处理

- 生产路径禁止对 `Result` / `Option` 调用 `unwrap()`、`expect()`；用 `?` 传播，
  并优先使用本仓库的上下文错误入口（`into_contextual()`、`context_error()`），
  保证错误携带原因与结构化 reason。
- `unwrap_or` / `unwrap_or_else` / `unwrap_or_default` 不属于 panic 路径，但兜底值
  必须就是正确答案，不得用来掩盖本应上报的错误。
- 禁止用 `.max(0)`、`as` 转型、`unwrap_or_default` 等方式静默吞掉异常值
  （负数、溢出、截断）；改用 `try_from` / `try_into` + 带原因的 error。
  无损放宽转换（如 `u32 -> u64`）不受此限。
- 仅当失败在逻辑上不可能、且 panic 可接受时（启动期配置、常量）才使用
  `expect("原因")`，必须写明原因。
- 测试、示例、benchmark 不受以上限制。

## 类型表达

- 类型宽度必须与真实可能性一致：不可能为 NULL 的值不要写成 `Option<T>`。
- 数据库的 NULL 语义在 SQL 层解决（聚合用 `if_null` / `COALESCE`），不要把 NULL
  泄漏成 Rust 侧的 `Option` 再兜底。
- 不要为了「防御」放宽类型或加 clamp；兜底值必须来自查询语义本身。
