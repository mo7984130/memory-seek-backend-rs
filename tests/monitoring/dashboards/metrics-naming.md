# Metrics 指标规范

适用范围：`domains/*`（auth / user / visual / backup / audit）与依赖库（common 多级缓存、
libs/oss、libs/email）。目标：每个对外操作都具备 `attempts` / `success` /
`duration_seconds` 三要素，指标名低基数、单位统一、可被 dashboard 直接消费。

> 错误分类（`{crate}:{func}:errors:{kind}` / `inc_error!`）已移除，暂不使用。

> 本文是**唯一事实来源**：新增或修改任何指标前，先在此登记；dashboard 侧的
> `jsonnet/lib/ops.libsonnet` 必须与本文保持一致（见
> [dashboard-design-spec.md](./dashboard-design-spec.md)）。

## 1. 分层与职责

| 层 | 位置 | 职责 |
|----|------|------|
| 埋点宏 | `common/src/macros/metrics/*` | `inc_counter!` / `set_gauge!` / `timed!` / `metrics_name!` / `MetricsTimer` / `GaugeGuard` |
| 服务入口宏 | `libs/common_macros` 的 `#[common_macros::metered]` | 自动产出 `attempts` / `duration_seconds` / `success` |
| HTTP 中间件 | `server/src/middlewares/metrics.rs` | `server.http.*` RED 指标，附 `module` 标签 |
| 系统采集 | `server/src/setup/bases/metrics/*` | `system.*` / `database.*` / `redis.*` / `server.build_info` |

## 2. 埋点契约

### 2.1 操作级：`#[metered]` + `#[instrument]`

每个对外 service 操作在入口叠加两个属性：`metered` 负责计数与耗时，`instrument`
提供 span（子步骤指标依赖 span 名）。

```rust
#[common_macros::metered(name = "get_collection_visuals")] // 缺省时用 rust 函数名
#[tracing::instrument(
    name = "get_collection_visuals",                       // 必须与 metered 的 name 一致
    skip_all,
    fields(user_id = %user_id)
)]
pub async fn get_visuals(
    state: &VisualState,
    user_id: UserId,
    collection_id: CollectionId,
    req: CollectionVisualCursorPageParam,
) -> Result<CollectionVisualView> {
    // 子步骤
    let ids = CollectionRepo::query_collection_visual_ids(state, user_id, collection_id, &req)
        .timed(metrics_name!("query_visual_ids"))
        .await?;

    // 失败分类
    let visuals = VisualService::load_visuals_info(state, user_id, &ids)
        .timed(metrics_name!("load_visuals_info"))
        .await?;

    Ok(visuals)
}
```

产出指标：

| 指标名 | 类型 | 来源 |
|--------|------|------|
| `visual:get_collection_visuals:attempts` | counter | `#[metered]` |
| `visual:get_collection_visuals:duration_seconds` | histogram | `#[metered]` |
| `visual:get_collection_visuals:success` | counter | `#[metered]`（`Result::is_ok` 时 +1） |
| `visual:get_collection_visuals:query_visual_ids` | histogram | `.timed(metrics_name!("step"))` |

约定：

1. `metered` 仅支持 `async fn ... -> Result<...>`；不满足时编译报错。
2. `{func}` 段对操作级指标取 **`metered` 的 name**（缺省为 rust 函数名）；对子步骤
   （`.timed(metrics_name!(...))` / `inc_counter!` / `set_gauge!`）取
   **当前 span 名**。两者必须一致，否则 attempts 与子步骤会落到不同 `{func}` 下。
   函数名过泛或语义不清晰时，用 `metered(name = "...")` + `instrument(name = "...")`
   成对指定（如 `like` → `like_comment`、`get_visuals` → `get_collection_visuals`）。
3. 子步骤必须处于 `instrument` span 内（通常即 service 入口），否则会取到中间件的
   `request` span，退化为 `{crate}:request:*` 并互相冲突。

### 2.2 子步骤计时

耗时子步骤有两种写法，**指标名结构不同**，查询时需对应：

| 写法 | 代码 | 指标名 | 查询 |
|------|------|--------|------|
| 写法一 | `.timed(metrics_name!("db_query"))` | `{crate}:{func}:db_query` | `<名>_sum / <名>_count` |
| 写法二 | `timed!("validate_image", { ... })` | `{crate}:{func}:validate_image:duration_seconds` | `<名>_sum / <名>_count` |
| 写法三 | `MetricsTimer::start(metrics_name!("download_batch"))` | `{crate}:{func}:download_batch` | 同写法一 |

> 写法二带显式 `:duration_seconds` 后缀，写法一/三不带。混用项目内两种写法时，
> 必须先确认步骤属于哪种，再决定查询名是否包含 `:duration_seconds`。

### 2.3 计数与 gauge

```rust
inc_counter!("visuals_processed", 1);            // counter
set_gauge!("batch", batch_idx as f64);          // gauge（瞬时值）
set_gauge!("mode", 1.0, "mode" => "full");      // gauge + 标签
let _guard = GaugeGuard::start(metrics_name!("running")); // 进入 +1，drop 时 -1
```

### 2.4 手动埋点（不走 `metered`）

一次性任务、循环批处理等不适合操作级四要素的场景，可手动组合
`metrics_group!(name)` / `metrics_success!(name)` 与子步骤宏。示例见
`visual:face_compute`（`domains/visual/src/services/face_service.rs`）。

## 3. 命名规范

### 3.1 业务指标

使用英文冒号 `:` 分隔层级：

```
{crate}:{func}:{step}
```

- `{crate}` — 模块 crate 名（`auth` / `user` / `visual` / `backup`）
- `{func}` — 操作名，取 `#[metered(name = "...")]` 或 rust 函数名（见 2.1）
- `{step}` — 操作步骤 / 子步骤名，如 `attempts` / `success` / `duration_seconds` /
  `db_query` / `db_transaction` / `cache_get_or_load` / `s3_upload` 等，取自固定词表
  （见第 8 节完整清单）

### 3.2 系统指标（server）

系统指标使用点号分隔，Prometheus 导出时点号转换为下划线：

```
{crate}.{category}.{name}   →  Prometheus: {crate}_{category}_{name}
```

例如 `system.cpu.usage` → `system_cpu_usage`。

> 冒号 `:` 在 Prometheus 指标名中是合法字符（保留给 recording rule），因此业务指标
> `auth:login:attempts` 导出后保持不变；只有 `.` 会被替换为 `_`。点号体系中的 counter
> 统一以 `_total` 结尾（`server.http.requests_total` → `server_http_requests_total`），
> 以满足 Prometheus / Grafana 对 counter 约定的识别。

### 3.3 依赖指标前缀

依赖库指标第一段使用**逻辑前缀**（硬编码，非 `CARGO_PKG_NAME`）：
`cache:` / `oss:` / `email:`，其余遵循 `{prefix}:{op}:{step}`。

## 4. 指标类型、单位与分桶

| 类型 | 说明 | Prometheus 导出名 |
|------|------|-------------------|
| counter | 累积计数（调用量、成功数、处理数、错误数） | `{name}`（原样） |
| gauge | 瞬时值（并发度、模式、批次、进度） | `{name}`（原样） |
| histogram | 耗时分布 | `{name}_bucket` / `{name}_sum` / `{name}_count` |

- **耗时单位统一为秒**（`MetricsTimer` 记录 `as_secs_f64` / `Instant::elapsed`）；
  Grafana 面板如需毫秒展示，查询末尾 `* 1000`。
- 所有 histogram 统一分桶（秒）：`0.01 / 0.05 / 0.1 / 0.3 / 0.5 / 1 / 2 / 5 / 10 / 30`，
  由 `metrics-exporter-prometheus` 的 `set_buckets` 全局设置。
- 基础设施指标采集周期默认 5 秒，通过 `config.yml` 的 `metrics.interval_seconds` 配置。

## 5. 标签白名单

标签只允许**低基数**键，且值必须来自固定词表：

```
method / route（路由 pattern）/ module / status_class / kind / op / mode
```

- `route` 取自 `axum::extract::MatchedPath`（如 `/visuals/:id`），未匹配回退 `unmatched`。
- `module` 由路由前缀归类（`classify_module`，固定词表，见第 6 节）。
- **禁止**将真实 ID、用户输入、文件名等动态值写入标签。

## 6. HTTP 请求级指标（RED）

由 `server/src/middlewares/metrics.rs` 中间件统一采集，挂在 router 最外层，覆盖所有
进入的请求（含 404 / CORS 提前返回）。指标归 `server.http.*` 系统体系：

| 指标名 | 类型 | Labels | 含义 |
|--------|------|--------|------|
| `server.http.requests_total` | counter | `method`、`route`、`module`、`status_class` | 请求量 |
| `server.http.duration_seconds` | histogram | `method`、`route`、`module` | 请求耗时（秒） |
| `server.http.in_flight` | gauge | - | 当前在途请求数 |

- `module` 按路由前缀归类：`/auth/*` → `auth`、`/user/*` → `user`、`/visual/*` → `visual`、
  `/admin/backup/*` → `backup`、其余 `/admin/*` → `audit`，其余（`/health`、`/hello`、
  `unmatched` 等）→ `other`。
  模块 dashboard 的 HTTP 行按 `module` 过滤，全局视图（system dashboard）不过滤。
- Prometheus 导出名：`server_http_requests_total`、`server_http_duration_seconds_bucket`。

## 7. 依赖级指标

### 多级缓存（外部 crate `multi-level-cache`，feature: `metrics`）

统一缓存组件 `MultiLevelCache` 来自依赖 crate `multi-level-cache`，分为三级：
L1 本地 moka → L2 Redis → L3 数据库（loader）。
每个缓存实例按 `cache:{name}:{layer}:{op}` 命名，`{name}` 为实例名。

| 指标名 | 类型 | 含义 |
|--------|------|------|
| `cache:{name}:l1:hits` | counter | L1 命中次数 |
| `cache:{name}:l1:misses` | counter | L1 未命中次数 |
| `cache:{name}:l2:hits` | counter | L2 命中次数 |
| `cache:{name}:l2:misses` | counter | L2 未命中次数 |
| `cache:{name}:db:loads` | counter | 穿透 L2 直达数据库的加载次数 |
| `cache:{name}:l1:get:duration_seconds` | histogram | L1 查询耗时（命中时记录） |
| `cache:{name}:l2:get:duration_seconds` | histogram | L2 查询耗时（MGET） |
| `cache:{name}:db:load:duration_seconds` | histogram | loader（数据库）加载耗时 |
| `cache:{name}:l1:entries` | gauge | L1 当前条目数 |

命中率计算：`rate(cache:{name}:l1:hits[5m]) / (rate(cache:{name}:l1:hits[5m]) + rate(cache:{name}:l1:misses[5m])) * 100`。

#### 实例清单

| 实例名 `{name}` | 缓存类型 | 数据源 | 写时失效点 |
|-----------------|----------|--------|------------|
| `user_info` | 用户信息批量（`UserInfoRow`） | `get_user_info_batch` | `change_nickname` / `update_avatar` / `logout` |
| `user_info_single` | 用户信息单查（完整 `UserInfo`） | `get_user_info` | 同上（独立实例，key 与批量一致） |
| `visual_info` | 照片信息（`VisualRecord`，按 visual_id 单一 key） | `load_visuals_info` | `delete_visuals` |
| `visual_like` | 单条点赞状态（`CachedVisualLike`） | `load_visuals_info`（批量读） | `like_visual` / `unlike_visual`（写状态） |
| `visual_cursor_ids` | 用户照片游标页（`CursorPage<VisualId, ()>`） | `query_visual_cursor_ids` | `upload_visual` / `delete_visuals`（`invalidate_visual_cursor_ids`） |
| `visual_dimensions` | 照片尺寸 `(w, h)` | 裁剪 token 宽高查询 | `delete_visuals` |
| `timeline_stat` | 月度统计整表（`Vec<MonthStat>`） | `get_monthly_stats` | `upload_visual` / `delete_visuals` |

> visual_info 缓存内容为 `VisualRecord`，不含按浏览者签发的 token（token 在读取时按浏览者
> 动态生成），因此键只按照片拆分，同一照片所有浏览者共享一份缓存。

### oss（libs/oss，feature: `metrics`）

每个操作埋 `requests` / `errors` / `duration_seconds` 三种指标，op 取值
`put` / `put_stream` / `delete` / `delete_batch` / `get` / `get_stream` /
`get_with_process` / `stream_with_process` / `sign` / `exists`：

| 指标名 | 类型 | 含义 |
|--------|------|------|
| `oss:{op}:requests` | counter | 调用量 |
| `oss:{op}:errors` | counter | 失败次数 |
| `oss:{op}:duration_seconds` | histogram | 单次操作耗时 |
| `oss:{op}:retries` | counter | 429 退避重试次数（`retry.rs` 内自增） |

### email（libs/email，feature: `metrics`）

| 指标名 | 类型 | 含义 |
|--------|------|------|
| `email:send:attempts` | counter | 发送尝试次数 |
| `email:send:success` | counter | 发送成功次数 |
| `email:send:errors:smtp` | counter | 发送失败次数 |
| `email:send:duration_seconds` | histogram | 发送耗时 |

### backup（domains/backup，feature: `metrics`）

备份流程在 `service.rs` 埋点，op 取 `scheduled` / `manual`：

| 指标名 | 类型 | 含义 |
|--------|------|------|
| `backup:{op}:attempts` | counter | 备份任务启动次数 |
| `backup:{op}:success` | counter | 备份任务成功次数 |
| `backup:{op}:duration_seconds` | histogram | 备份任务总耗时 |
| `backup:{op}:tables_exported` | counter | 成功导出的表数 |
| `backup:{op}:tables_failed` | counter | 失败的表数 |
| `backup:{op}:cleaned` | counter | GFS 清理删除的备份数 |

数据层子步骤（histogram，`{op}` 取调用方 span 名）：`backup:{op}:export`（导出单表）、
`backup:restore:restore_local`（恢复导入）。

恢复流程（`restore`，`POST /admin/backup/restore`）单独埋点：

| 指标名 | 类型 | 含义 |
|--------|------|------|
| `backup:restore:attempts` | counter | 恢复启动次数 |
| `backup:restore:success` | counter | 恢复成功次数 |
| `backup:restore:duration_seconds` | histogram | 恢复总耗时 |
| `backup:restore:tables` | counter | 恢复的表数 |
| `backup:restore:rows` | counter | 恢复的行数 |

## 8. 完整指标清单

### auth 模块

| 函数 | 指标 |
|------|------|
| login | `auth:login:attempts` `auth:login:success` `auth:login:duration_seconds`<br>`auth:login:db_query` `auth:login:acquire_permit` `auth:login:verify_password` `auth:login:redis_set` `auth:login:db_update` |
| register | `auth:register:attempts` `auth:register:success` `auth:register:duration_seconds`<br>`auth:register:verify_email_code` `auth:register:verify_inviter_code` `auth:register:hash_password` `auth:register:db_insert` `auth:register:redis_delete` |
| send_email_code | `auth:send_email_code:attempts` `auth:send_email_code:success` `auth:send_email_code:duration_seconds`<br>`auth:send_email_code:redis_set` `auth:send_email_code:send_message` `auth:send_email_code:acquire_permit` |
| refresh_access_token | `auth:refresh_access_token:attempts` `auth:refresh_access_token:success` `auth:refresh_access_token:duration_seconds`<br>`auth:refresh_access_token:verify_token` `auth:refresh_access_token:set_token` |

### user 模块

| 函数 | 指标 |
|------|------|
| get_user_info | `user:get_user_info:attempts` `user:get_user_info:success` `user:get_user_info:duration_seconds`<br>`user:get_user_info:cache_get_or_load` |
| generate_inviter_code | `user:generate_inviter_code:attempts` `user:generate_inviter_code:success` `user:generate_inviter_code:duration_seconds`<br>`user:generate_inviter_code:redis_set` |
| change_nickname | `user:change_nickname:attempts` `user:change_nickname:success` `user:change_nickname:duration_seconds`<br>`user:change_nickname:db_update` `user:change_nickname:db_transaction` `user:change_nickname:cache_invalidate` `user:change_nickname:cache_invalidate_single` |
| update_avatar | `user:update_avatar:attempts` `user:update_avatar:success` `user:update_avatar:duration_seconds`<br>`user:update_avatar:validate_image:duration_seconds` `user:update_avatar:s3_upload` `user:update_avatar:db_transaction` `user:update_avatar:cache_invalidate` `user:update_avatar:cache_invalidate_single` `user:update_avatar:s3_delete` |
| change_password | `user:change_password:attempts` `user:change_password:success` `user:change_password:duration_seconds`<br>`user:change_password:db_query` `user:change_password:acquire_permit` `user:change_password:verify_password` `user:change_password:hash_password` `user:change_password:db_transaction`<br>登出清理（内部复用 `do_logout`）：`user:change_password:redis_delete` `user:change_password:cache_invalidate` `user:change_password:cache_invalidate_single` |
| logout | `user:logout:attempts` `user:logout:success` `user:logout:duration_seconds`<br>`user:logout:db_transaction` `user:logout:redis_delete` `user:logout:cache_invalidate` `user:logout:cache_invalidate_single` |
| get_user_info_batch | `user:get_user_info_batch:attempts` `user:get_user_info_batch:success` `user:get_user_info_batch:duration_seconds`<br>`user:get_user_info_batch:cache_get_or_load_batch` |

### visual 模块

| 函数 | 指标 |
|------|------|
| get_visual_cursor_page | `visual:get_visual_cursor_page:attempts` `visual:get_visual_cursor_page:success` `visual:get_visual_cursor_page:duration_seconds`<br>`visual:get_visual_cursor_page:find_cursor_page_ids` `visual:get_visual_cursor_page:load_visuals_info` |
| upload_visual | `visual:upload_visual:attempts` `visual:upload_visual:success` `visual:upload_visual:duration_seconds`<br>`visual:upload_visual:validate_visual:duration_seconds` `visual:upload_visual:md5_hash:duration_seconds` `visual:upload_visual:s3_upload` `visual:upload_visual:db_insert`<br>`visual:upload_visual:cache_get_or_load` `visual:upload_visual:cache_put` `visual:upload_visual:cache_invalidate` |
| exists_by_md5_batch | `visual:exists_by_md5_batch:attempts` `visual:exists_by_md5_batch:success` `visual:exists_by_md5_batch:duration_seconds` |
| delete_visuals | `visual:delete_visuals:attempts` `visual:delete_visuals:success` `visual:delete_visuals:duration_seconds`<br>`visual:delete_visuals:db_transaction` `visual:delete_visuals:s3_delete_batch` `visual:delete_visuals:cache_invalidate` `visual:delete_visuals:cache_invalidate_dimensions` `visual:delete_visuals:cache_invalidate_timeline` |
| download_image | `visual:download_image:attempts` `visual:download_image:success` `visual:download_image:duration_seconds`<br>`visual:download_image:s3_download_process` `visual:download_image:s3_download_stream` |
| get_collection_list | `visual:get_collection_list:attempts` `visual:get_collection_list:success` `visual:get_collection_list:duration_seconds`<br>`visual:get_collection_list:query_by_user_id` |
| create_collection | `visual:create_collection:attempts` `visual:create_collection:success` `visual:create_collection:duration_seconds`<br>`visual:create_collection:db_insert` |
| update_collection_info | `visual:update_collection_info:attempts` `visual:update_collection_info:success` `visual:update_collection_info:duration_seconds`<br>`visual:update_collection_info:db_update` |
| delete_collection | `visual:delete_collection:attempts` `visual:delete_collection:success` `visual:delete_collection:duration_seconds`<br>`visual:delete_collection:db_transaction` |
| get_collections_by_visual | `visual:get_collections_by_visual:attempts` `visual:get_collections_by_visual:success` `visual:get_collections_by_visual:duration_seconds` |
| get_collection_visuals | `visual:get_collection_visuals:attempts` `visual:get_collection_visuals:success` `visual:get_collection_visuals:duration_seconds`<br>`visual:get_collection_visuals:query_visual_ids` `visual:get_collection_visuals:load_visuals_info` |
| add_collection_visuals | `visual:add_collection_visuals:attempts` `visual:add_collection_visuals:success` `visual:add_collection_visuals:duration_seconds`<br>`visual:add_collection_visuals:auth_check` `visual:add_collection_visuals:db_transaction` |
| remove_collection_visuals | `visual:remove_collection_visuals:attempts` `visual:remove_collection_visuals:success` `visual:remove_collection_visuals:duration_seconds`<br>`visual:remove_collection_visuals:db_transaction` |
| publish_comment | `visual:publish_comment:attempts` `visual:publish_comment:success` `visual:publish_comment:duration_seconds`<br>`visual:publish_comment:db_transaction` |
| get_comment_cursor_page | `visual:get_comment_cursor_page:attempts` `visual:get_comment_cursor_page:success` `visual:get_comment_cursor_page:duration_seconds`<br>`visual:get_comment_cursor_page:query_hot_comments` `visual:get_comment_cursor_page:query_by_visual_id` `visual:get_comment_cursor_page:query_is_like` |
| delete_comment | `visual:delete_comment:attempts` `visual:delete_comment:success` `visual:delete_comment:duration_seconds`<br>`visual:delete_comment:db_transaction` |
| like_comment | `visual:like_comment:attempts` `visual:like_comment:success` `visual:like_comment:duration_seconds`<br>`visual:like_comment:db_transaction` |
| unlike_comment | `visual:unlike_comment:attempts` `visual:unlike_comment:success` `visual:unlike_comment:duration_seconds`<br>`visual:unlike_comment:db_transaction` |
| like_visual | `visual:like_visual:attempts` `visual:like_visual:success` `visual:like_visual:duration_seconds`<br>`visual:like_visual:db_transaction` |
| unlike_visual | `visual:unlike_visual:attempts` `visual:unlike_visual:success` `visual:unlike_visual:duration_seconds`<br>`visual:unlike_visual:db_transaction` |
| get_user_liked_visuals | `visual:get_user_liked_visuals:attempts` `visual:get_user_liked_visuals:success` `visual:get_user_liked_visuals:duration_seconds`<br>`visual:get_user_liked_visuals:query_ids` `visual:get_user_liked_visuals:load_visuals_info` |
| rename_person | `visual:rename_person:attempts` `visual:rename_person:success` `visual:rename_person:duration_seconds`<br>`visual:rename_person:db_transaction` |
| merge_person | `visual:merge_person:attempts` `visual:merge_person:success` `visual:merge_person:duration_seconds`<br>`visual:merge_person:db_transaction` `visual:merge_person:cache_get_or_load` |
| get_persons | `visual:get_persons:attempts` `visual:get_persons:success` `visual:get_persons:duration_seconds`<br>`visual:get_persons:query_page` |
| search_persons | `visual:search_persons:attempts` `visual:search_persons:success` `visual:search_persons:duration_seconds`<br>`visual:search_persons:query_search` |
| change_face_belonging | `visual:change_face_belonging:attempts` `visual:change_face_belonging:success` `visual:change_face_belonging:duration_seconds`<br>`visual:change_face_belonging:db_transaction` |
| delete_face | `visual:delete_face:attempts` `visual:delete_face:success` `visual:delete_face:duration_seconds` |
| delete_faces_batch | `visual:delete_faces_batch:attempts` `visual:delete_faces_batch:success` `visual:delete_faces_batch:duration_seconds` |
| face_compute（人脸） | counter：`visual:face_compute:attempts` `visual:face_compute:success` `visual:face_compute:visuals_processed` `visual:face_compute:faces_detected` `visual:face_compute:no_face_visuals`<br>gauge：`visual:face_compute:running` `visual:face_compute:mode`（labels `full` / `incremental`）`visual:face_compute:batch` `visual:face_compute:total_visuals` `visual:face_compute:total_faces` `visual:face_compute:total_no_face`<br>histogram：`visual:face_compute:duration_seconds` `visual:face_compute:query` `visual:face_compute:download_batch` `visual:face_compute:visual_download` `visual:face_compute:visual_decode` `visual:face_compute:visual_detect` `visual:face_compute:insert` |
| get_monthly_stats | `visual:get_monthly_stats:attempts` `visual:get_monthly_stats:success` `visual:get_monthly_stats:duration_seconds`<br>`visual:get_monthly_stats:cache_get_or_load` |
| get_visual_info | `visual:get_visual_info:attempts` `visual:get_visual_info:success` `visual:get_visual_info:duration_seconds`<br>`visual:get_visual_info:load_visuals_info` |
| get_person_visuals | `visual:get_person_visuals:attempts` `visual:get_person_visuals:success` `visual:get_person_visuals:duration_seconds`<br>`visual:get_person_visuals:query_visual_ids` `visual:get_person_visuals:load_visuals_info` |
| get_faces_by_visual_id | `visual:get_faces_by_visual_id:attempts` `visual:get_faces_by_visual_id:success` `visual:get_faces_by_visual_id:duration_seconds` |
| get_unassigned_face_visuals | `visual:get_unassigned_face_visuals:attempts` `visual:get_unassigned_face_visuals:success` `visual:get_unassigned_face_visuals:duration_seconds`<br>`visual:get_unassigned_face_visuals:query_unassigned_face_visual_ids` `visual:get_unassigned_face_visuals:load_visuals_info` |
| delete_person | `visual:delete_person:attempts` `visual:delete_person:success` `visual:delete_person:duration_seconds`<br>`visual:delete_person:db_transaction` |
| person_full_scan | `visual:person_full_scan:attempts` `visual:person_full_scan:success` `visual:person_full_scan:duration_seconds` |
| person_secondary_cluster | `visual:person_secondary_cluster:attempts` `visual:person_secondary_cluster:success` `visual:person_secondary_cluster:duration_seconds` |

### audit 模块

`audit` 已接入 metrics（feature: `metrics`）。操作级指标由 `AuditQueryer` / `AuditRecorder`
埋点产出（`query_*` 仅在 `controller` feature 下编译，`db_insert` 仅在 `recording` 下产出）：

| 函数 | 指标 |
|------|------|
| query_stats | `audit:query_stats:attempts` `audit:query_stats:success` `audit:query_stats:duration_seconds` `audit:query_stats:db_query` |
| query_top | `audit:query_top:attempts` `audit:query_top:success` `audit:query_top:duration_seconds` `audit:query_top:db_query` |
| query_events | `audit:query_events:attempts` `audit:query_events:success` `audit:query_events:duration_seconds` `audit:query_events:db_query` |
| append | `audit:append:attempts` `audit:append:success` `audit:append:duration_seconds` |
| append_many | `audit:append_many:attempts` `audit:append_many:success` `audit:append_many:duration_seconds` `audit:append_many:db_insert`（`recording`） |

HTTP 视角仍由 `server.http.*{module="audit"}` 覆盖（`/admin/audits`；`/admin/backup/*` 归 `module="backup"`）。

### server 系统指标

| 指标名（代码） | Prometheus 导出名 | 类型 | 含义 |
|----------------|-------------------|------|------|
| `system.cpu.usage` | `system_cpu_usage` | gauge | 系统 CPU 使用率 |
| `system.cpu.cores` | `system_cpu_cores` | gauge | 逻辑 CPU 核数 |
| `system.memory.total` | `system_memory_total` | gauge | 系统总内存（字节） |
| `system.memory.used` | `system_memory_used` | gauge | 系统已用内存（字节） |
| `system.cpu.process_usage` | `system_cpu_process_usage` | gauge | 进程 CPU 使用率 |
| `system.memory.process_usage` | `system_memory_process_usage` | gauge | 进程内存（字节） |
| `system.disk.total` | `system_disk_total` | gauge | 工作目录所在分区总容量（字节） |
| `system.disk.used` | `system_disk_used` | gauge | 工作目录所在分区已用（字节） |
| `database.connections.active` | `database_connections_active` | gauge | 数据库连接池活跃连接 |
| `database.connections.idle` | `database_connections_idle` | gauge | 数据库连接池空闲连接 |
| `database.connections.max` | `database_connections_max` | gauge | 数据库连接池上限 |
| `redis.connections.active` | `redis_connections_active` | gauge | Redis 连接池活跃连接 |
| `redis.connections.idle` | `redis_connections_idle` | gauge | Redis 连接池空闲连接 |
| `redis.connections.waiting` | `redis_connections_waiting` | gauge | Redis 连接池等待连接 |
| `server.build_info` | `server_build_info` | gauge=1 | labels：`version` / `commit`，用于版本追踪 |
| `server.http.requests_total` | `server_http_requests_total` | counter | labels：`method` / `route` / `module` / `status_class`，HTTP 请求量 |
| `server.http.duration_seconds` | `server_http_duration_seconds` | histogram | labels：`method` / `route` / `module`，HTTP 请求耗时 |
| `server.http.in_flight` | `server_http_in_flight` | gauge | 在途请求数 |

## 9. feature 接线

各 crate 的 `metrics` feature 负责开启本域埋点；server 按启用的业务域级联。

| crate | feature 定义 | 依赖 |
|-------|--------------|------|
| common | `metrics` | `dep:metrics` |
| oss | `metrics` | `common/metrics` |
| email | `metrics` | `common/metrics` |
| auth | `metrics` | `common/metrics`、`email/metrics` |
| user | `metrics` | `common/metrics`、`multi-level-cache/metrics`、`oss/metrics` |
| visual | `metrics` | `common/metrics`、`multi-level-cache/metrics`、`oss/metrics` |
| backup | `metrics` | `common/metrics`、`oss/metrics` |
| audit | `metrics` | `common/metrics` |
| server | `metrics` | `common/metrics`、`dep:metrics`、`dep:sysinfo`、`dep:metrics-exporter-prometheus`、`visual?/metrics`、`user?/metrics`、`auth?/metrics`、`backup?/metrics`、`audit?/metrics`、`oss?/metrics`、`email?/metrics` |

> `oss` / `email` / `audit` 已提供独立 `metrics` feature：`oss` 由 user / visual / backup
> 级联，`email` 由 auth 级联，`audit` 由 server 按可选依赖级联；server 还级联
> `oss?/metrics` / `email?/metrics` / `audit?/metrics`。新增业务域时，务必同步在 server 的
> `metrics` feature 中追加 `{domain}?/metrics`。

## 10. 新增 / 修改检查清单

新增一个可观测操作：

1. service 入口加 `#[common_macros::metered]`（必要时 `name = "..."`）+ 同名
   `#[tracing::instrument(name = "...", skip_all, fields(...))]`。
2. 子步骤：`.timed(metrics_name!("{step}"))`（写法一）或 `timed!("{step}", ...)`（写法二）。
3. 需要瞬时值时用 `set_gauge!` / `GaugeGuard`。
4. 在本文「完整指标清单」登记该操作的全部指标。
5. 在 `tests/monitoring/dashboards/jsonnet/lib/ops.libsonnet` 追加该操作，再执行
   `sh tests/monitoring/dashboards/jsonnet/generate.sh`。

新增一个业务域：

1. crate 暴露 `metrics = ["common/metrics", ...]`。
2. 在 server 的 `metrics` feature 中追加 `{domain}?/metrics`。
3. 按上述流程为每个对外操作埋点，并在本文登记。

## 11. 命名要点

1. 业务指标第一段为模块 crate 名（`auth` / `user` / `visual` / `backup`）。
2. 操作名来自 `#[metered(name)]`（缺省为 rust 函数名）；子步骤来自 span 名，二者
   必须一致（见 2.1 约定 2）。
3. 依赖指标使用逻辑前缀 `cache:` / `oss:` / `email:`。
4. 系统指标使用点号层级，导出后变为下划线；其中 counter 以 `_total` 结尾。
5. 每个 service 操作都需有 `#[tracing::instrument]`，否则子步骤宏会取到中间件的
   `request` span，退化为 `{crate}:request:*` 并互相冲突。
6. 耗时单位统一为**秒**；Grafana 面板如需毫秒显示，查询需 `* 1000`。

## 12. 已知缺口

- **backup 口径**：部分表失败时整体仍记 `success`（以 `tables_failed` 衡量部分失败）；
  `cleanup` 失败仅记日志、不影响 `success`。

## 更新记录

- 2026-09-12: 补齐剩余缺口。子步骤：auth `login:db_update` / `register:redis_delete` /
  `send_email_code:acquire_permit`；visual `get_visual_info:load_visuals_info` /
  `get_user_liked_visuals:load_visuals_info` / `change_face_belonging:db_transaction`；
  user `update_avatar` 补偿删除 `s3_delete`。oss 补 `sign` / `exists` op 指标；
  `/admin/backup/*` 归入 `module="backup"`；audit `append` / `append_many` 消除重复计数；
  修复 register 的 `email_code_prefix` 按字节切片 panic；visual / cache / system dashboard
  迁移进 jsonnet（cache 耗时单位由 µs 统一为 ms），`generate.sh` 覆盖全部 5 个 dashboard。

- 2026-09-12: 移除错误分类系统（删除 `inc_error!` 宏与全部调用，文档同步移除
  `errors:{kind}` 相关章节与清单条目）；补齐 visual repo 层子步骤埋点；backup 数据层
  （`export` / `restore_local`）埋点；audit 域接入 `metrics`（`audit:query_*` /
  `audit:append*`，feature 级联 `audit?/metrics`）；按代码实际修正 visual 人物相关
  子步骤并移除未落地的 `record*` / `face_compute` 冗余条目。
- 2026-09-12: 修复审计发现的 P0 缺口。`oss` / `email` 新增 `metrics` feature 并补齐埋点
  （`oss:{op}:requests/errors/duration_seconds/retries`、`email:send:*`）；feature 级联更新；
  补 `server.build_info`；修正 `redis.connections.active` 语义；事件消费者补命名 span
  （照片上传/删除/人脸链路子步骤不再落 `visual:unknown:*`）；`delete_faces` 统一为
  `delete_faces_batch`；`delete_person` 补 `metered`；person 内部方法补 `instrument`；
  backup `restore` 补四要素；`change_password` 不再重复计入 `user:logout:*`。
- 2026-09-12: 完善为可落地规范。补充「分层与职责」「埋点契约」（`#[metered]` +
  `#[instrument]`，含 name 一致性规则与两种计时写法）；明确 `{func}` 来源与标签白名单；
  新增 feature 接线表、新增/修改检查清单；修正 user 模块清单（`db_query`→
  `cache_get_or_load`、`redis_cache`→`cache_get_or_load_batch`、`redis_delete`→
  `cache_invalidate`、`db_update`→`db_transaction`，补 `cache_invalidate_single`）；
  标注 audit 模块暂无操作级指标。
- 2026-09-10: HTTP 请求级指标新增 `module` 标签（按路由前缀归类：auth / user / visual /
  audit，未知归 other），模块 dashboard 的 HTTP 行按 `module` 过滤。
- 2026-08-01: 重构命名规范。废弃 summary 式 `duration_quantile`，改用原生 histogram
  （`{name}_bucket/_sum/_count`）；业务指标统一为 `{crate}:{func}:{step}`，操作名来自
  tracing span；补齐 visual 模块 `#[tracing::instrument]`；人脸计算指标迁移为
  `visual:face_compute:*`；对语义模糊或冲突的操作显式命名 span
  （`publish_comment` / `like_visual` / `get_collection_visuals` 等）
- 2026-08-09: 完善监控体系。
  - 新增 HTTP 请求级指标（RED）中间件与 `server.http.*` 指标体系；
  - 新增错误分类规范与 `inc_error!` 宏（`{crate}:{func}:errors:{kind}`）；
  - 补齐 face_compute 已声明指标（`visuals_processed` / `faces_detected` / `no_face_visuals`、
    `errors:download` 等、`running` / `mode` / `batch` / `total_*` gauge、批次子步骤 histogram）；
  - 新增依赖级指标：oss（`oss:{op}:requests/errors/duration_seconds/retries`）、
    email（`email:send:*`）、backup（`backup:{op}:*`）；
  - 基础设施增强：`system.cpu.cores` / `system.disk.*` / `database.connections.max` /
    `server.build_info`；采集周期 `metrics.interval_seconds` 可配置；histogram 统一分桶。
  - feature 级联：server `metrics` 现在级联 `visual/user/auth/email/oss/backup` 各域 metrics。
  - 补齐行为审计、人物管理、人脸归属/删除共 9 个操作埋点；为 auth 登录/注册、
    visual 上传补充 `errors:{kind}` 分类；修复 `metrics_group!` 显式函数名形式
    产生 `{crate}:{func}::duration_seconds` 双冒号的缺陷。
- 2026-08-09: 引入统一多级缓存组件 `MultiLevelCache`（L1 moka → L2 Redis → L3 数据库），
  新增依赖级指标 `cache:{name}:{layer}:{op}`（命中率 / 耗时 / L1 容量 / 穿透加载数）；
  user 与 visual 模块的缓存读写迁移至新组件，原 `redis_delete` / `redis_delete_cache` /
  `redis_cache` 步骤分别更名为 `cache_invalidate` / `cache_get_or_load_batch`，
  visual 删除补充 `cache_invalidate` 步骤。
- 2026-08-09: 扩展缓存接入范围。visual_info 缓存键改为按照片单一 key（缓存不含浏览者
  token，token 读取时动态生成），消除同照片多浏览者的缓存冗余；新增 5 个缓存实例：
  `user_info_single`（用户单查）、`timeline_stat`（月度统计）、`visual_dimensions`（尺寸）、
  `visual_md5`（去重）、`person`（人物轻量摘要）；upload/delete/rename/merge/face 变更等
  写操作补齐缓存失效；dashboard 改用 `$cache` 模板变量按实例切换。
