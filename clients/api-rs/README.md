# memory-seek-api-rs

Memory Seek 后端的 **Rust API 客户端**。作为 `memory-seek-backend` 工作区成员置于
`clients/api-rs`,契约类型直接复用同工作区的 `types-*` crate。

TS 客户端 `memory-seek-api-ts` 仍留在 `memory-seek-api` 仓库,与本 crate 语言并列。

## 设计要点

- **运行时无关**:不绑定 tokio 等运行时,异步由调用方提供的执行器驱动。
- **HTTP 库无关**:通过 `Transport` trait 由调用方注入真实实现;并提供可选 feature
  `reqwest-transport` 下的开箱即用 `ReqwestTransport`。
- **标准 HTTP 类型**:请求/响应描述复用 [`http`](https://docs.rs/http) crate 的
  `Method` / `HeaderMap` / `StatusCode`(仅类型, 不含客户端或运行时), 与 reqwest / hyper 等零成本互转。
- **存储可注入**:通过 `Storage` trait 由调用方注入 token 存储。内置 `MemoryStorage`
  (临时/测试),并提供可选 feature `file-storage` 下的跨平台 `FileStorage`;钥匙串等可由调用方自行实现。
- **类型复用**:请求/响应 DTO 直接复用后端契约 crate(`types-core` / `types-identity` /
  `types-visual` / `types-visual-token` / `types-audit`),不重复定义,杜绝前后端漂移。

后端契约 crate 通过工作区内部路径依赖(见根 `Cargo.toml` 的 `[workspace.dependencies]`),
不再依赖仓库外的同级目录布局。

## 用法

```rust
use memory_seek_api::{
    ApiConfig, Client, MemoryStorage, Transport, HttpRequest, HttpResponse, TransportError,
};

// 1) 提供 HTTP 实现(示例:任意 HTTP 库;或直接使用内置 reqwest transport,见下)
struct MyTransport { /* reqwest::Client 等 */ }

#[async_trait::async_trait]
impl Transport for MyTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        // 把 request.method / url / headers / body 交给真实 HTTP 库发送,
        // 再把结果包装成 HttpResponse { status, headers, body }。
        todo!()
    }
}

// 2) 提供存储实现或使用内置 MemoryStorage;transport / storage 按值传入
let client = Client::new(
    ApiConfig::new("http://127.0.0.1:8080/api"),
    MyTransport { /* ... */ },
    MemoryStorage::new(),
);

// 3) 登录并保存登录态(之后请求会自动注入 Authorization、401 自动刷新)
let login = memory_seek_api::api::auth::login(&client, &request).await?;
client.auth_storage().set_login_result(&login)?;

// 4) 调用业务接口
let me = memory_seek_api::api::user::get_me(&client).await?;
let visuals = memory_seek_api::api::visual::get_visuals(&client, None, Some(20), None, None).await?;
```

> `Client` 是 `Clone` 的轻量句柄(内部 `Arc`),可跨任务 / 线程共享;`transport` /
> `storage` 按值传入具体实现,若需多个 `Client` 复用同一实现,可直接传
> `Arc<T>` / `Box<dyn ...>`(见 `Transport` / `Storage` 的 blanket impl)。

### 内置 reqwest 传输层

启用 `reqwest-transport` feature 后可用开箱即用的 `ReqwestTransport`
(**需自行提供 tokio 等异步运行时**);超时 / 连接池 / TLS 等由传入的 `reqwest::Client` 决定:

```toml
memory-seek-api-rs = { path = "...", features = ["reqwest-transport"] }
```

```rust
use memory_seek_api::{ApiConfig, Client, MemoryStorage, ReqwestTransport};

// 开箱即用:内部用默认 reqwest::Client
let transport = ReqwestTransport::new();
// 或自定义超时 / 连接池 / TLS:ReqwestTransport::with_client(reqwest::Client::builder()...build()?)

let client = Client::new(
    ApiConfig::new("http://127.0.0.1:8080/api"),
    transport,
    MemoryStorage::new(),
);
```

### 内置文件存储

启用 `file-storage` feature 后可用跨平台的 `FileStorage`(全部键值存于单个
`credentials.json`,写入采用「临时文件 + 原子替换」):

```toml
memory-seek-api-rs = { path = "...", features = ["file-storage"] }
```

```rust
use memory_seek_api::{ApiConfig, Client, FileStorage};

// 方式一:落到各平台用户数据目录的 <app> 下,文件名固定 credentials.json
let storage = FileStorage::for_app("MemorySeek")?;
// 方式二:同上,但自定义凭据文件名(同一应用区分多份凭据)
// let storage = FileStorage::for_app_with_file_name("MemorySeek", "other.json")?;
// 方式三:显式指定完整路径(便于测试或自定义位置),构造时即读入磁盘内容
// let storage = FileStorage::new("/tmp/memoryseek/credentials.json")?;

let client = Client::new(ApiConfig::new("http://127.0.0.1:8080/api"), transport, storage);
```

`for_app("<app>")` 的落盘位置:

| 平台 | 路径 |
| --- | --- |
| Linux | `$XDG_DATA_HOME/<app>`(缺省 `~/.local/share/<app>`) |
| macOS | `~/Library/Application Support/<app>` |
| Windows | `%APPDATA%\<app>\data` |

> 说明:token 明文落盘,属已知取舍;如需系统钥匙串,可实现 `Storage` 自行接入。
> `Storage` 各方法返回 `Result<_, StorageError>`;`AuthStorage` 把底层存储错误透传,
> 而 `loginResult` 无法解析时按「未登录」处理。

接口分组与 TS 客户端一一对应:

| 分组 | 路径 |
| --- | --- |
| 认证 | `api::auth` |
| 用户 | `api::user` |
| 影像 / 收藏夹 / 评论 / 时间线 / 点赞 | `api::visual` 及 `api::visual::{collection, comment, timeline, like}` |
| 审计(管理端) | `api::audit` |

## 当前范围

最小可用版本已覆盖上述分组。**暂未包含**人脸 / 人物接口(`/visual/face`、`/visual/person`):
其后端 DTO 被 gate 在 `types-visual` 的 `face-engine` feature 下,该 feature 会拖入
`sea-orm` + `insight-face-rs` 等重依赖,与"轻量客户端"目标冲突。后续可通过后端解 gate
或在本 crate 内提供轻量镜像类型来补齐。

## 验证

```bash
cargo fmt --all -- --check
cargo clippy --release -p memory-seek-api-rs --all-targets -- -D warnings
cargo test --release -p memory-seek-api-rs
# 含可选文件存储 / reqwest 传输层
cargo clippy --release -p memory-seek-api-rs --all-targets --features "file-storage,reqwest-transport" -- -D warnings
cargo test --release -p memory-seek-api-rs --features "file-storage,reqwest-transport"
```
