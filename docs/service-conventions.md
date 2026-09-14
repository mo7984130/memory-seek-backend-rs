# Service 层约定

统一 domains 各 service / controller 的传参、tracing 与 metrics 埋点。

## 函数签名

顺序固定:`state: &XxxState` → 身份(`user_id` / `admin`) → 资源 ID(`{resource}_id`) → `req: XxxParam` → `file_data`。

- 请求体参数一律命名 `req`(controller 侧对齐);**不在签名里解构 DTO**。
- 身份:普通用户 `user_id`,管理操作用 `admin`(避免 `viewer` 等变体,除非同时存在归属者与查看者)。
- 返回类型统一 `common::Result<T>`,不用 `Result<T, AppError>`。

## tracing

所有 `pub async` **必须**带 `#[tracing::instrument]` —— 否则 `metrics_group!()` 会取到中间件的
`request` span,导致指标名冲突。

```rust
#[tracing::instrument(
    skip_all,
    fields(user_id = %user_id, count = %req.photo_ids.len())
)]
```

- `skip_all` + `fields` 显式记录;不记录 `req` 整体。
- 字段命名无前缀、语义化(`account` / `count`);一律 `字段 = %值` 显式写法,禁止裸 `fields(user_id)`。
- 敏感字段脱敏:密码不记录,验证码只记前缀(`email_code_prefix`)。
- 函数名过泛或冲突时用 `#[instrument(name = "...")]` 显式命名(命名规范见 `docs/dashboards/metrics-naming.md`)。

## metrics

- `metrics_group!()` 必须是函数体**首条语句**(在 instrument span 内),保证 `metrics_name!` 取到正确的函数名。
- `metrics_success!()` 在成功返回前调用一次,用无参写法。
- 异步任务模式在真正执行任务的 inner 函数上埋点,并用 `name = "..."` 使其语义化。

## 日志

- **禁止** `info!(status = "success", ...)` 之类成功日志(已由 `metrics_success!()` 覆盖)。
- 保留流程性/信息性日志(任务进度、管理员触发等)。

## controller

- extractor 变量与 service 形参逐一对齐:`ValidatedJson(req)` / `ValidatedQuery(req)` /
  `Extension(user_id)` / `Path({resource}_id)`。
- 返回统一 `.await.to_r_ok()` 链式;无返回值时 `Ok(()).to_r_ok()`。

## schema 与迁移

- 表结构由 `types::db_init::init_db` 的 schema sync 按 entity 创建,**只增不改**:
  已存在的表与列不会被 ALTER,老库的加列/改默认值等需**手工** `ALTER` 对齐。
- 会被插入省略的 NOT NULL 列(时间戳、计数等)必须在 entity 上声明默认值:
  `#[sea_orm(default_expr = "sea_orm::sea_query::Expr::current_timestamp()")]`(时间戳)、
  `#[sea_orm(default_value = 0)]`(计数),新库由 sync 直接建出默认值。
  回归测试见 `types/src/lib.rs` 的 `column_default_tests`。
