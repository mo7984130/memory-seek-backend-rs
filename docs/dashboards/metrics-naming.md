# Metrics 指标命名规范

## 命名格式

指标名使用英文冒号 `:` 分隔，格式为：

```
{crate}:{func}:{step}
```

- `{crate}` — 模块 crate 名（`auth` / `user` / `photo` / `server`）
- `{func}` — 操作名。默认取 tracing span 名（由 `#[tracing::instrument]` 生成）；
  函数名过泛或与其他操作冲突时，通过 `#[instrument(name = "...")]` 显式指定语义化名称
- `{step}` — 操作步骤，如 `attempts` / `success` / `duration_seconds` / `db_query` 等

### 系统指标（server 模块）

系统指标使用点号分隔，Prometheus 导出时点号会被转换为下划线：

```
{crate}.{category}.{name}   →  Prometheus: {crate}_{category}_{name}
```

例如 `system.cpu.usage` → `system_cpu_usage`。

> 冒号 `:` 在 Prometheus 指标名中是合法字符（保留给 recording rule 使用），因此业务指标
> `auth:login:attempts` 导出后保持不变；只有 `.` 会被替换为 `_`。

## 指标类型

| 类型 | 说明 | Prometheus 导出名 |
|------|------|-------------------|
| counter | 累积计数（调用量、成功数、处理数） | `{name}`（原样） |
| gauge | 瞬时值（并发度、模式、批次、累计值） | `{name}`（原样） |
| histogram | 耗时分布 | `{name}_bucket` / `{name}_sum` / `{name}_count` |

## 计时指标命名

计时器通过 `MetricsTimer` 记录，单位为**秒**（`as_secs_f64`）。存在两种写法：

### 写法一：`.timed(metrics_name!("step"))`（Future 扩展）

```rust
// 主耗时（metrics_group! 自动产生）
metrics_group!();                          // → {crate}:{func}:duration_seconds
// 子步骤
future.timed(metrics_name!("db_query"))    // → {crate}:{func}:db_query
```

指标名**不带** `:duration_seconds` 后缀，Prometheus 自动附加 `_sum` / `_count` / `_bucket`：

```
auth:login:db_query_bucket{le="..."}   # histogram 分桶
auth:login:db_query_sum                # 总耗时（秒）
auth:login:db_query_count              # 请求计数
```

### 写法二：`timed!("step", ...)` 宏 / 显式 `:duration_seconds`

```rust
timed!("validate_image", { ... })      // → {crate}:{func}:validate_image:duration_seconds
MetricsTimer::start(metrics_name!("download_batch:duration_seconds"))
                                       // → {crate}:{func}:download_batch:duration_seconds
```

指标名**带** `:duration_seconds` 后缀：

```
photo:face_compute:photo_download:duration_seconds_sum
photo:face_compute:photo_download:duration_seconds_count
```

> 两种写法混用时，查询前需确认该步骤属于哪种形式，以确定是否包含 `:duration_seconds` 段。

## 核心指标

每个埋点操作自动生成以下指标：

| 指标名 | 类型 | 来源 |
|--------|------|------|
| `{crate}:{func}:attempts` | counter | `metrics_group!()` |
| `{crate}:{func}:duration_seconds` | histogram | `metrics_group!()` |
| `{crate}:{func}:success` | counter | `metrics_success!()` |
| `{crate}:{func}:{step}` | histogram | `.timed(metrics_name!("{step}"))` |
| `{crate}:{func}:{step}:duration_seconds` | histogram | `timed!("{step}", ...)` |
| `{crate}:{func}:{step}` | counter/gauge | `inc_counter!` / `set_gauge!` |

## HTTP 请求级指标（RED）

由 `server/src/middlewares/metrics.rs` 中间件统一采集，挂在 router 最外层，
覆盖所有进入的请求（含 404 / CORS 提前返回）。指标归 `server.http.*` 系统体系：

| 指标名 | 类型 | Labels | 含义 |
|--------|------|--------|------|
| `server.http.requests_total` | counter | `method`、`route`、`status_class` | 请求量 |
| `server.http.duration_seconds` | histogram | `method`、`route` | 请求耗时（秒） |
| `server.http.in_flight` | gauge | - | 当前在途请求数 |

- `route` 取自 `axum::extract::MatchedPath`（路由 pattern，如 `/api/v1/photos/:id`），
  未匹配时回退为 `unmatched`。**禁止**将真实 ID 等动态值写入标签。
- 低基数标签白名单：`method`、`route`（pattern）、`status_class`、`kind`、`op`、`mode`。
- Prometheus 导出名：`server_http_requests_total`、`server_http_duration_seconds_bucket`。
  点号系统指标的 counter 统一以 `_total` 结尾，满足 Prometheus 命名约定（消除
  Grafana PromQL 对 `rate()` 目标的 counter 类型警告）；业务冒号指标保留原名。

## 错误分类规范

业务操作统一按 `attempts` / `success` / `errors:{kind}` 三元组埋点，
错误走 `{crate}:{func}:errors:{kind}` counter（不再依赖 success/attempts 差值推断失败）。

`kind` 白名单：

```
db / redis / s3 / smtp / validation / auth / not_found / conflict / internal
领域专属：download / decode / detect / insert / export / save / cleanup
```

通过 `inc_error!("{kind}")` 宏埋点（自动取当前 span 名）或
`inc_error!("{func}", "{kind}")`（显式指定函数名）。

成功率面板：`rate({crate}:{func}:success[5m]) / rate({crate}:{func}:attempts[5m]) * 100`；
错误分布面板：`sum(rate({crate}:{func}:errors:*[5m])) by (kind)`。

## 依赖级指标

### 多级缓存（common，feature: `metrics`）

统一缓存组件 `MultiLevelCache` 分为三级：L1 本地 moka → L2 Redis → L3 数据库（loader）。
每个缓存实例按 `cache:{name}:{layer}:{op}` 命名，`{name}` 为实例名（如 `user_info` / `photo_info`）。

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

### oss（libs/oss，feature: `metrics`）

每个操作埋 `requests` / `errors` / `duration_seconds` 三种指标，op 取值
`put` / `delete` / `delete_batch` / `get` / `get_stream` / `get_with_process` /
`stream_with_process` / `sign`：

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

备份流程在 `runner.rs` 埋点，op 取 `scheduled` / `manual`：

| 指标名 | 类型 | 含义 |
|--------|------|------|
| `backup:{op}:attempts` | counter | 备份任务启动次数 |
| `backup:{op}:success` | counter | 备份任务成功次数 |
| `backup:{op}:errors:{kind}` | counter | `export` / `save` / `cleanup` 分类错误 |
| `backup:{op}:duration_seconds` | histogram | 备份任务总耗时 |
| `backup:{op}:tables_exported` | counter | 成功导出的表数 |
| `backup:{op}:tables_failed` | counter | 失败的表数 |
| `backup:{op}:cleaned` | counter | GFS 清理删除的备份数 |

## 完整指标清单

### auth 模块

| 函数 | 指标 |
|------|------|
| login | `auth:login:attempts` `auth:login:success` `auth:login:duration_seconds`<br>`auth:login:db_query` `auth:login:acquire_permit` `auth:login:verify_password` `auth:login:redis_set`<br>`auth:login:errors:auth`（账号不存在 / 密码错误） |
| register | `auth:register:attempts` `auth:register:success` `auth:register:duration_seconds`<br>`auth:register:verify_email_code` `auth:register:verify_inviter_code` `auth:register:hash_password` `auth:register:db_insert`<br>`auth:register:errors:validation`（密码不一致 / 验证码错误 / 邀请码错误） |
| send_email_code | `auth:send_email_code:attempts` `auth:send_email_code:success` `auth:send_email_code:duration_seconds`<br>`auth:send_email_code:redis_set` `auth:send_email_code:send_message` |
| refresh_access_token | `auth:refresh_access_token:attempts` `auth:refresh_access_token:success` `auth:refresh_access_token:duration_seconds`<br>`auth:refresh_access_token:verify_token` `auth:refresh_access_token:set_token` |

### user 模块

| 函数 | 指标 |
|------|------|
| get_user_info | `user:get_user_info:attempts` `user:get_user_info:success` `user:get_user_info:duration_seconds`<br>`user:get_user_info:db_query` |
| generate_inviter_code | `user:generate_inviter_code:attempts` `user:generate_inviter_code:success` `user:generate_inviter_code:duration_seconds`<br>`user:generate_inviter_code:redis_set` |
| change_nickname | `user:change_nickname:attempts` `user:change_nickname:success` `user:change_nickname:duration_seconds`<br>`user:change_nickname:db_update` `user:change_nickname:cache_invalidate` |
| update_avatar | `user:update_avatar:attempts` `user:update_avatar:success` `user:update_avatar:duration_seconds`<br>`user:update_avatar:validate_image:duration_seconds` `user:update_avatar:s3_upload` `user:update_avatar:db_transaction` `user:update_avatar:cache_invalidate` `user:update_avatar:s3_delete` |
| change_password | `user:change_password:attempts` `user:change_password:success` `user:change_password:duration_seconds`<br>`user:change_password:db_query` `user:change_password:acquire_permit` `user:change_password:verify_password` `user:change_password:hash_password` `user:change_password:db_update` |
| logout | `user:logout:attempts` `user:logout:success` `user:logout:duration_seconds`<br>`user:logout:db_update` `user:logout:redis_delete` `user:logout:cache_invalidate` |
| get_user_info_batch | `user:get_user_info_batch:attempts` `user:get_user_info_batch:success` `user:get_user_info_batch:duration_seconds`<br>`user:get_user_info_batch:cache_get_or_load_batch` |

### photo 模块

| 函数 | 指标 |
|------|------|
| get_photo_cursor_page | `photo:get_photo_cursor_page:attempts` `photo:get_photo_cursor_page:success` `photo:get_photo_cursor_page:duration_seconds`<br>`photo:get_photo_cursor_page:find_cursor_page_ids` `photo:get_photo_cursor_page:load_photos_info` |
| upload_photo | `photo:upload_photo:attempts` `photo:upload_photo:success` `photo:upload_photo:duration_seconds`<br>`photo:upload_photo:validate_photo:duration_seconds` `photo:upload_photo:md5_hash:duration_seconds` `photo:upload_photo:s3_upload` `photo:upload_photo:db_insert` |
| exists_by_md5_batch | `photo:exists_by_md5_batch:attempts` `photo:exists_by_md5_batch:success` `photo:exists_by_md5_batch:duration_seconds` |
| delete_photos | `photo:delete_photos:attempts` `photo:delete_photos:success` `photo:delete_photos:duration_seconds`<br>`photo:delete_photos:db_transaction` `photo:delete_photos:s3_delete_batch` `photo:delete_photos:cache_invalidate` |
| download_image | `photo:download_image:attempts` `photo:download_image:success` `photo:download_image:duration_seconds`<br>`photo:download_image:s3_download_process` `photo:download_image:s3_download_stream` |
| get_collection_list | `photo:get_collection_list:attempts` `photo:get_collection_list:success` `photo:get_collection_list:duration_seconds`<br>`photo:get_collection_list:query_by_user_id` |
| create_collection | `photo:create_collection:attempts` `photo:create_collection:success` `photo:create_collection:duration_seconds`<br>`photo:create_collection:db_insert` |
| update_collection_info | `photo:update_collection_info:attempts` `photo:update_collection_info:success` `photo:update_collection_info:duration_seconds`<br>`photo:update_collection_info:db_update` |
| delete_collection | `photo:delete_collection:attempts` `photo:delete_collection:success` `photo:delete_collection:duration_seconds`<br>`photo:delete_collection:db_transaction` |
| get_collections_by_photo | `photo:get_collections_by_photo:attempts` `photo:get_collections_by_photo:success` `photo:get_collections_by_photo:duration_seconds` |
| get_collection_photos | `photo:get_collection_photos:attempts` `photo:get_collection_photos:success` `photo:get_collection_photos:duration_seconds`<br>`photo:get_collection_photos:query_photo_ids` `photo:get_collection_photos:load_photos_info` |
| add_collection_photos | `photo:add_collection_photos:attempts` `photo:add_collection_photos:success` `photo:add_collection_photos:duration_seconds`<br>`photo:add_collection_photos:auth_check` `photo:add_collection_photos:db_transaction` |
| remove_collection_photos | `photo:remove_collection_photos:attempts` `photo:remove_collection_photos:success` `photo:remove_collection_photos:duration_seconds`<br>`photo:remove_collection_photos:db_transaction` |
| publish_comment | `photo:publish_comment:attempts` `photo:publish_comment:success` `photo:publish_comment:duration_seconds`<br>`photo:publish_comment:db_transaction:duration_seconds` |
| get_comment_cursor_page | `photo:get_comment_cursor_page:attempts` `photo:get_comment_cursor_page:success` `photo:get_comment_cursor_page:duration_seconds`<br>`photo:get_comment_cursor_page:query_hot_comments` `photo:get_comment_cursor_page:query_by_photo_id` `photo:get_comment_cursor_page:query_is_like` |
| delete_comment | `photo:delete_comment:attempts` `photo:delete_comment:success` `photo:delete_comment:duration_seconds`<br>`photo:delete_comment:db_transaction:duration_seconds` |
| like_comment | `photo:like_comment:attempts` `photo:like_comment:success` `photo:like_comment:duration_seconds`<br>`photo:like_comment:db_transaction:duration_seconds` |
| unlike_comment | `photo:unlike_comment:attempts` `photo:unlike_comment:success` `photo:unlike_comment:duration_seconds`<br>`photo:unlike_comment:db_transaction:duration_seconds` |
| like_photo | `photo:like_photo:attempts` `photo:like_photo:success` `photo:like_photo:duration_seconds`<br>`photo:like_photo:db_transaction:duration_seconds` |
| unlike_photo | `photo:unlike_photo:attempts` `photo:unlike_photo:success` `photo:unlike_photo:duration_seconds`<br>`photo:unlike_photo:db_transaction:duration_seconds` |
| get_user_liked_photos | `photo:get_user_liked_photos:attempts` `photo:get_user_liked_photos:success` `photo:get_user_liked_photos:duration_seconds`<br>`photo:get_user_liked_photos:query_ids` |
| record（行为审计） | `photo:record:attempts` `photo:record:success` `photo:record:errors:db` `photo:record:duration_seconds` |
| record_view_async（图片浏览） | `photo:record_view_async:attempts` `photo:record_view_async:success` `photo:record_view_async:errors:db` `photo:record_view_async:duration_seconds` |
| rename_person | `photo:rename_person:attempts` `photo:rename_person:success` `photo:rename_person:duration_seconds` |
| merge_person | `photo:merge_person:attempts` `photo:merge_person:success` `photo:merge_person:duration_seconds` |
| get_persons | `photo:get_persons:attempts` `photo:get_persons:success` `photo:get_persons:duration_seconds` |
| search_persons | `photo:search_persons:attempts` `photo:search_persons:success` `photo:search_persons:duration_seconds` |
| change_face_belonging | `photo:change_face_belonging:attempts` `photo:change_face_belonging:success` `photo:change_face_belonging:duration_seconds` |
| delete_face | `photo:delete_face:attempts` `photo:delete_face:success` `photo:delete_face:errors:conflict` `photo:delete_face:duration_seconds` |
| delete_faces_batch | `photo:delete_faces_batch:attempts` `photo:delete_faces_batch:success` `photo:delete_faces_batch:duration_seconds` |
| upload_photo | 补充错误分类：`photo:upload_photo:errors:validation` `photo:upload_photo:errors:conflict` `photo:upload_photo:errors:s3` `photo:upload_photo:errors:db` |
| face_compute（人脸） | counter：`photo:face_compute:attempts` `photo:face_compute:success` `photo:face_compute:photos_processed` `photo:face_compute:faces_detected` `photo:face_compute:no_face_photos` `photo:face_compute:errors:download` `photo:face_compute:errors:decode` `photo:face_compute:errors:detect` `photo:face_compute:errors:insert`<br>gauge：`photo:face_compute:running` `photo:face_compute:mode`（labels `full` / `incremental`）`photo:face_compute:batch` `photo:face_compute:total_photos` `photo:face_compute:total_faces` `photo:face_compute:total_no_face`<br>histogram：`photo:face_compute:duration_seconds` `photo:face_compute:cleanup:backup` `photo:face_compute:query` `photo:face_compute:download_batch` `photo:face_compute:photo_download` `photo:face_compute:photo_decode` `photo:face_compute:detect_batch` `photo:face_compute:photo_detect` `photo:face_compute:insert_phase` `photo:face_compute:insert` |
| get_monthly_stats | `photo:get_monthly_stats:attempts` `photo:get_monthly_stats:success` `photo:get_monthly_stats:duration_seconds`<br>`photo:get_monthly_stats:query_monthly_stats` |

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
| `server.http.requests_total` | `server_http_requests_total` | counter | labels：`method` / `route` / `status_class`，HTTP 请求量 |
| `server.http.duration_seconds` | `server_http_duration_seconds` | histogram | labels：`method` / `route`，HTTP 请求耗时 |
| `server.http.in_flight` | `server_http_in_flight` | gauge | 在途请求数 |

### 采集周期与分桶

- 基础设施指标采集周期默认 5 秒，通过 `config.yml` 的 `metrics.interval_seconds` 配置。
- 所有 histogram 统一分桶（秒）：`0.01 / 0.05 / 0.1 / 0.3 / 0.5 / 1 / 2 / 5 / 10 / 30`，
  由 `metrics-exporter-prometheus` 的 `set_buckets` 全局设置。

## 命名要点

1. 所有业务指标第一段为模块 crate 名（`auth` / `user` / `photo`）
2. 业务指标中间段为操作名（span 名）；函数名过泛或同名冲突时通过 `#[instrument(name = "...")]`
   显式指定语义化名称（如 `like_comment` / `unlike_comment` / `like_photo` / `unlike_photo`、
   `publish_comment` / `get_comment_cursor_page` / `face_compute` 等）
3. 系统指标使用点号层级，导出后变为下划线
4. 所有服务函数需有 `#[tracing::instrument]`，否则 `metrics_group!()` 会取到中间件
   的 `request` span，导致指标名退化为 `{crate}:request:*` 并互相冲突
5. 耗时单位统一为**秒**；Grafana 面板如需毫秒显示，查询需 `* 1000`

## 更新记录

- 2026-08-01: 重构命名规范。废弃 summary 式 `duration_quantile`，改用原生 histogram
  （`{name}_bucket/_sum/_count`）；业务指标统一为 `{crate}:{func}:{step}`，操作名来自
  tracing span；补齐 photo 模块 `#[tracing::instrument]`；人脸计算指标迁移为
  `photo:face_compute:*`；对语义模糊或冲突的操作显式命名 span
  （`publish_comment` / `like_photo` / `get_collection_photos` 等）
- 2026-08-09: 完善监控体系。
  - 新增 HTTP 请求级指标（RED）中间件与 `server.http.*` 指标体系；
  - 新增错误分类规范与 `inc_error!` 宏（`{crate}:{func}:errors:{kind}`）；
  - 补齐 face_compute 已声明指标（`photos_processed` / `faces_detected` / `no_face_photos`、
    `errors:download` 等、`running` / `mode` / `batch` / `total_*` gauge、批次子步骤 histogram）；
  - 新增依赖级指标：oss（`oss:{op}:requests/errors/duration_seconds/retries`）、
    email（`email:send:*`）、backup（`backup:{op}:*`）；
  - 基础设施增强：`system.cpu.cores` / `system.disk.*` / `database.connections.max` /
    `server.build_info`；采集周期 `metrics.interval_seconds` 可配置；histogram 统一分桶。
  - feature 级联：server `metrics` 现在级联 `photo/user/auth/email/oss/backup` 各域 metrics。
  - 补齐行为审计、人物管理、人脸归属/删除共 9 个操作埋点；为 auth 登录/注册、
    photo 上传补充 `errors:{kind}` 分类；修复 `metrics_group!` 显式函数名形式
    产生 `{crate}:{func}::duration_seconds` 双冒号的缺陷。
- 2026-08-09: 引入统一多级缓存组件 `MultiLevelCache`（L1 moka → L2 Redis → L3 数据库），
  新增依赖级指标 `cache:{name}:{layer}:{op}`（命中率 / 耗时 / L1 容量 / 穿透加载数）；
  user 与 photo 模块的缓存读写迁移至新组件，原 `redis_delete` / `redis_delete_cache` /
  `redis_cache` 步骤分别更名为 `cache_invalidate` / `cache_get_or_load_batch`，
  photo 删除补充 `cache_invalidate` 步骤。
