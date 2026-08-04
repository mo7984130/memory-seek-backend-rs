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

## 完整指标清单

### auth 模块

| 函数 | 指标 |
|------|------|
| login | `auth:login:attempts` `auth:login:success` `auth:login:duration_seconds`<br>`auth:login:db_query` `auth:login:acquire_permit` `auth:login:verify_password` `auth:login:redis_set` |
| register | `auth:register:attempts` `auth:register:success` `auth:register:duration_seconds`<br>`auth:register:verify_email_code` `auth:register:verify_inviter_code` `auth:register:hash_password` `auth:register:db_insert` |
| send_email_code | `auth:send_email_code:attempts` `auth:send_email_code:success` `auth:send_email_code:duration_seconds`<br>`auth:send_email_code:redis_set` `auth:send_email_code:send_message` |
| refresh_access_token | `auth:refresh_access_token:attempts` `auth:refresh_access_token:success` `auth:refresh_access_token:duration_seconds`<br>`auth:refresh_access_token:verify_token` `auth:refresh_access_token:set_token` |

### user 模块

| 函数 | 指标 |
|------|------|
| get_user_info | `user:get_user_info:attempts` `user:get_user_info:success` `user:get_user_info:duration_seconds`<br>`user:get_user_info:db_query` |
| generate_inviter_code | `user:generate_inviter_code:attempts` `user:generate_inviter_code:success` `user:generate_inviter_code:duration_seconds`<br>`user:generate_inviter_code:redis_set` |
| change_nickname | `user:change_nickname:attempts` `user:change_nickname:success` `user:change_nickname:duration_seconds`<br>`user:change_nickname:db_update` `user:change_nickname:redis_delete` |
| update_avatar | `user:update_avatar:attempts` `user:update_avatar:success` `user:update_avatar:duration_seconds`<br>`user:update_avatar:validate_image:duration_seconds` `user:update_avatar:s3_upload` `user:update_avatar:db_transaction` `user:update_avatar:redis_delete` `user:update_avatar:s3_delete` |
| change_password | `user:change_password:attempts` `user:change_password:success` `user:change_password:duration_seconds`<br>`user:change_password:db_query` `user:change_password:acquire_permit` `user:change_password:verify_password` `user:change_password:hash_password` `user:change_password:db_update` |
| logout | `user:logout:attempts` `user:logout:success` `user:logout:duration_seconds`<br>`user:logout:db_update` `user:logout:redis_delete` `user:logout:redis_delete_cache` |
| get_user_info_batch | `user:get_user_info_batch:attempts` `user:get_user_info_batch:success` `user:get_user_info_batch:duration_seconds`<br>`user:get_user_info_batch:redis_cache` |

### photo 模块

| 函数 | 指标 |
|------|------|
| get_photo_cursor_page | `photo:get_photo_cursor_page:attempts` `photo:get_photo_cursor_page:success` `photo:get_photo_cursor_page:duration_seconds`<br>`photo:get_photo_cursor_page:find_cursor_page_ids` `photo:get_photo_cursor_page:load_photos_info` |
| upload_photo | `photo:upload_photo:attempts` `photo:upload_photo:success` `photo:upload_photo:duration_seconds`<br>`photo:upload_photo:validate_photo:duration_seconds` `photo:upload_photo:md5_hash:duration_seconds` `photo:upload_photo:s3_upload` `photo:upload_photo:db_insert` |
| exists_by_md5_batch | `photo:exists_by_md5_batch:attempts` `photo:exists_by_md5_batch:success` `photo:exists_by_md5_batch:duration_seconds` |
| delete_photos | `photo:delete_photos:attempts` `photo:delete_photos:success` `photo:delete_photos:duration_seconds`<br>`photo:delete_photos:db_transaction` `photo:delete_photos:s3_delete_batch` |
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
| face_compute（人脸） | counter：`photo:face_compute:attempts` `photo:face_compute:success` `photo:face_compute:photos_processed` `photo:face_compute:faces_detected` `photo:face_compute:no_face_photos` `photo:face_compute:errors:download` `photo:face_compute:errors:decode` `photo:face_compute:errors:detect` `photo:face_compute:errors:insert`<br>gauge：`photo:face_compute:running` `photo:face_compute:mode` `photo:face_compute:batch` `photo:face_compute:total_photos` `photo:face_compute:total_faces` `photo:face_compute:total_no_face`<br>histogram：`photo:face_compute:duration_seconds` `photo:face_compute:cleanup:duration_seconds` `photo:face_compute:cleanup:backup` `photo:face_compute:cleanup:truncate:duration_seconds` `photo:face_compute:batch_loop` `photo:face_compute:query` `photo:face_compute:download_batch:duration_seconds` `photo:face_compute:photo_download:duration_seconds` `photo:face_compute:photo_decode:duration_seconds` `photo:face_compute:detect_batch:duration_seconds` `photo:face_compute:photo_detect:duration_seconds` `photo:face_compute:insert_phase:duration_seconds` `photo:face_compute:insert:duration_seconds` |
| get_monthly_stats | `photo:get_monthly_stats:attempts` `photo:get_monthly_stats:success` `photo:get_monthly_stats:duration_seconds`<br>`photo:get_monthly_stats:query_monthly_stats` |

### server 系统指标

| 指标名（代码） | Prometheus 导出名 | 类型 | 含义 |
|----------------|-------------------|------|------|
| `system.cpu.usage` | `system_cpu_usage` | gauge | 系统 CPU 使用率 |
| `system.memory.total` | `system_memory_total` | gauge | 系统总内存（字节） |
| `system.memory.used` | `system_memory_used` | gauge | 系统已用内存（字节） |
| `system.cpu.process_usage` | `system_cpu_process_usage` | gauge | 进程 CPU 使用率 |
| `system.memory.process_usage` | `system_memory_process_usage` | gauge | 进程内存（字节） |
| `database.connections.active` | `database_connections_active` | gauge | 数据库连接池活跃连接 |
| `database.connections.idle` | `database_connections_idle` | gauge | 数据库连接池空闲连接 |
| `redis.connections.active` | `redis_connections_active` | gauge | Redis 连接池活跃连接 |
| `redis.connections.idle` | `redis_connections_idle` | gauge | Redis 连接池空闲连接 |
| `redis.connections.waiting` | `redis_connections_waiting` | gauge | Redis 连接池等待连接 |

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
