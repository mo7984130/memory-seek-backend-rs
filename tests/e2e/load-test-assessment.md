# 高并发正确性压测评估

> 评估范围：`.github/workflows/load.yml`、`.github/workflows/e2e.yml`、`tests/e2e/**`、`memseek-test` 运行时。
> 目标：判断现有压测能否暴露高并发下的「数据一致性/竞态」与「过载/稳定性」问题，并给出改造方案。

## 1. 结论

**现有压测是「性能/稳定性回归」工具，不是「并发正确性」工具。**

| 类别 | 能否发现 | 说明 |
| --- | --- | --- |
| 显式失败（5xx、409、超时） | ✅ 能 | 计入 `failures` / `timeouts`，影响通过率 |
| 过载与资源耗尽 | ⚠️ 部分能 | 能观察超时/503/吞吐下降；连接池、缓存、限流指标只归档不判定 |
| 静默竞态（计数漂移、缓存不一致、重复副作用、幂等失效） | ❌ 不能 | 断言全部是「单请求视角」，没有聚合不变量对账 |
| 同账号/同资源的并发写冲突 | ❌ 不能 | 32 个并发任务被刻意分配不同账号、不同资源，设计上在**回避**竞争 |
| 多实例一致性（L1 缓存、任务 fencing） | ❌ 不能 | 压测只跑单实例，且 admin/backup/audit 未编译进被测 server |
| 优雅关闭 | ❌ 不能 | 压测流程不发送 SIGINT |

量化现状：e2e 共 **64 个场景**，压测模式排除 4 个断言型/CPU 密集型场景后跑 60 个；
其中 **0 个**聚合不变量断言，**0 个**同账号同资源竞争用例，admin/backup/audit 域 **0 场景**。

## 2. 现状

### 2.1 运行链路

- `.github/workflows/load.yml`：`workflow_dispatch` / `push main` / 每周一定时。
- 模式：`E2E__RUN__MODE=duration`，默认 `15s/场景`，并发固定 `32`。
- 场景**串行**执行，总时长 ≈ 场景数 × 15s；没有混合负载，也没有阶梯加压。
- 被测 server 由 `.github/actions/e2e-stack` 构建，`taskset` 限 2 核；依赖 PG / Valkey / MailHog / rustfs 单机容器。
- 客户端统一 5s 超时，`run.backoff_ms=0`（超时后立即重试，压测下更易暴露过载）。

### 2.2 并发模型

`memseek-test` 的 `ScenarioManager`（`~/.cargo/registry/.../memseek-test-0.23.0/src/manager.rs`）：

- 每个场景创建 `concurrency` 个并发任务，`task_index` 全局编号 `0..31`。
- 所有任务通过 `join_all` 在同一 Tokio runtime 上协作调度；`workers` 默认 1。
- 场景间串行：前一个场景的 32 个任务全部结束，才开始下一个场景。
- 每轮骨架：`setup → run → validate → teardown`，`teardown` 是 finally 语义。

关键点：**并发度只体现在「同一场景内 32 个不同用户」**。账号按 `task_index` 分配：

- auth / 登录：`loadtest_{index+1}`
- visual：`loadtest_visual_{index+1}`
- user 写操作：`uit_user_{index+1}` / `uit_me_*` / `uit_pwd_*` / `uit_logout_*`（按场景分池）

`tests/e2e/src/scenarios/user/session.rs:22` 的注释写得很明确：不同账号池是为了「与写场景隔离避免并发竞态」。
也就是说，当前并发模型是**隔离型并发**，不是**对抗型并发**。

### 2.3 判定口径

`tests/e2e/ci/load_compare.py`：

- 绝对阈值（会失败）：有效通过率 ≥ 99%、超时率 ≤ 0.5%、收尾失败率 ≤ 2%、无中断场景。
- 基线对比（只标注）：单场景 P95 涨幅 > 20%、整体 RPS 低于基线 0.9 倍。
- `failures + validate_failures + timeouts` 统一从通过率里扣；**不区分错误类型**。
- Prometheus 快照只作为 artifact 上传，不参与判定。
- 压测本身允许 `max_failure_rate=1%`，即最多 1% 的请求失败不会让 workflow 失败。

### 2.4 数据与环境

- 种子：`preprea` 每次清库重灌，`visual_users=2000`，但实际只用前 32 个账号。
- 每轮数据基本自建自清：上传按 `task_index + round + 纳秒` 生成唯一内容；收藏夹、评论、点赞各自回收。
- `load.yml` 的 server features 为 `metrics,auth,user,visual,face-engine`，**不含 admin、backup、audit、audit-recording**。

## 3. 全模块覆盖矩阵

| 模块 | 场景数 | 压测中 | 并发隔离方式 | 共享资源竞争 | 现有断言 | 并发缺口 |
| --- | ---: | ---: | --- | --- | --- | --- |
| auth | 9 | 9 | 每 task 独立账号/邮箱 | 无（同账号被隔离） | token 落库、刷新后新 token、验证码链路 | 同账号并发登录、并发刷新、同邮箱并发发码 |
| user | 21 | 21 | uit 各子池按场景隔离 | 无（同账号被隔离） | 昵称落库 + 单次读缓存、头像 S3 回查、登出后的 token 失效 | 同账号并发改昵称/换头像/改密、并发登出 |
| visual 互动 | 15 | 13 | 每 task 独立用户 | **部分**：like 共享 `seed_file_2/3/4/7/8`，comment 共享 `seed_file_6` | 只校验「我这条记录」存在 | like_count、comment_count、缓存一致性 |
| visual 资源 | 18 | 14 | 每 task 自建影像/收藏夹 | 无 | 单资源 CRUD、token 绑定、S3 最终删除 | 同内容并发上传、同影像并发删除+读取 |
| system | 1 | 1 | — | — | `/metrics` 关键前缀存在 | 指标数值正确性 |
| admin | 0 | 0 | — | — | — | 任务领取、fencing、取消、失联回收 |
| backup | 0 | 0 | — | — | — | advisory lock、operation_lock、恢复中取消 |
| audit | 0 | 0 | — | — | — | 并发写审计、记录分片 |

压测排除的 4 个场景：`UploadDuplicateVisualScenario`、`VideoDerivativeScenario`、
`DownloadImageThumbnailScenario`、`DownloadImageCropScenario`。

## 4. 现有断言为什么抓不到静默竞态

### 4.1 聚合计数只断言「> 0」

- `tests/e2e/src/scenarios/visual/like.rs:106`：点赞后只查 `(user_id, visual_id)` 这一行是否存在，不看 `visual.like_count`。
- `tests/e2e/src/scenarios/visual/comment.rs:125-129`：评论后只断言 `comment_count > 0`，不校验等值。
- `tests/e2e/src/scenarios/visual/detail.rs:77-82`：详情只比对 name/size/width/height，不比对 `like_count` / `comment_count`。
- `tests/e2e/src/scenarios/visual/collection.rs:387-392`：只校验单次 add 后 count=1，且是每任务自建收藏夹。

结果：即使 32 个用户对同一张影像的点赞计数少加/多加、缓存里长期是旧值，压测仍会通过。

### 4.2 点赞/评论的「共享」只是部分共享

- `LikeVisualScenario`：32 个不同用户并发点赞**同一张** `seed_file_2_1`，这是目前唯一像热点的场景；但校验只看自己的行。
- `LikeCommentScenario`：每个 task 在 setup 里**各自发一条新评论**再点赞，评论级计数完全没有共享竞争。
- `GetCommentsScenario` 与 `PublishCommentScenario` 刻意用不同 ordinal，注释说明是为了避免长跑下被挤出首页（`comment.rs:65-72`）。

### 4.3 上传哈希去重存在 TOCTOU，但场景刚好绕开

`domains/visual/src/services/visual_service/upload.rs:188` 先 `exists_by_hash`，随后
`VisualRepo::insert_visual`（`:239`）再插入；两步之间没有锁，也没有 `ON CONFLICT` 兜底。

- 并发上传**相同内容**时，两个请求都可能通过存在性检查，后插入者撞 `visual_visual.hash` 唯一约束，按当前错误映射会变成 500，而不是业务上的 400 去重。
- 当前 `UploadDuplicateVisualScenario` 每任务内容唯一，且压测直接排除该场景，所以永远压不到这个窗口。

### 4.4 时间线统计的「先 UPDATE 再 INSERT」

`domains/visual/src/mappers/timeline_stat_mapper.rs:24-49`：

1. `UPDATE ... SET count = count + 1 WHERE date_str = ?`
2. 如果 `rows_affected == 0`，再 `INSERT (count = 1)`

新月份首次并发上传时，多个请求会同时走到 INSERT，其中一个撞 `date_str` 主键。
种子数据预置了时间线月份，正常压测不会触发；跨月切换或空库冷启动时才会暴露。

### 4.5 多级缓存的一致性窗口

先澄清一个容易误读的点：**当前 `invalidate` 是会删二级缓存（Redis L2）的**。
`multi-level-cache` 的实现是：

```rust
// ~/.cargo/registry/.../multi-level-cache-0.1.1/src/cache.rs:406
pub async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
    self.local.invalidate(&key.to_string()).await;  // L1：本实例
    self.backend.del(key).await?;                   // L2：Redis DEL
    Ok(())
}
```

真正的缺口有三层，按影响从大到小：

1. **其他实例的 L1 不会被删**。每个实例的 `local` 是各自进程内的 Moka 缓存，
   `invalidate` 只能清自己那份；其它实例最长陈旧 `cache.local_ttl_secs`（默认 60s）。
   单实例压测连这一层都暴露不了。
2. **L2 删除失败被吞掉**。视觉/用户侧调用点普遍是 `.emit_if_err()` 或 `let _ =`：
   - `domains/visual/src/repo/visual_repo.rs:103-113`（`invalidate_visual_info`）
   - `domains/visual/src/repo/count_cache.rs:24-31`（`invalidate`）
   - `domains/user/src/repo/user_repo.rs:189-203`（`invalidate_user_info`）

   Redis 超时/连接池耗尽时，L1 已清、L2 仍是旧值，请求继续返回旧数据，且只有一条日志/指标。
3. **`get_or_load` 无锁回填（缓存击穿 + 旧值回填）**：
   - 多个读者同时 miss，会并发打 DB（无 single-flight）。
   - 更严重的是「读旧值 → 写者提交并 invalidate → 读者回填」：旧值被重新写进 L2，
     可存活一个 L2 TTL（视觉/用户缓存为 24h，时间线统计为 1h）。
   - 现有 `ChangeNicknameScenario` 的 read-after-write 只覆盖「自己写、自己立刻读」，
     没有「A 读旧值、B 写入、A 回填」的交错。

`Example.config.yaml:35-45` 已提供 `MEMORY_SEEK_CACHE__ENABLED=false`，可用来把
「DB 竞态」与「缓存竞态」分开定位。

### 4.6 账号级写操作没有并发覆盖

| 路径 | 代码位置 | 并发风险 | 现有覆盖 |
| --- | --- | --- | --- |
| 登录 | `domains/auth/src/services/auth_service.rs:104-131` | 先写 Redis access_token，再写 DB refresh_token；同账号并发登录会互相覆盖，可能留下孤立的 access_token | 每 task 独立账号 |
| 刷新 | `domains/auth/src/services/auth_service.rs` | 同 refresh_token 并发刷新（是否轮换、是否幂等）未验证 | 每 task 独立账号 |
| 改密 | `domains/user/src/services/user_service.rs:170-210` | 两个并发改密都能通过旧密码校验，后写覆盖前写 | `uit_pwd_*` 每 task 唯一 |
| 换头像 | `domains/user/src/services/user_service.rs:145-157` | 并发替换时，后写者返回的 `old_key` 可能是前写者的新 key，导致先返回的 token 指向已删除对象 | 每 task 独立账号 |
| 登出 | `domains/user/src/services/user_service.rs` | 登出与并发刷新/读取交错 | `uit_logout_*` 每 task 唯一 |

### 4.7 admin / backup 的并发模型完全没有被压

`domains/admin/README.md` 描述的多实例模型（部分唯一索引、`FOR UPDATE SKIP LOCKED`、
`run_id` fencing、心跳回收、跨实例取消）和 `domains/backup/src/service.rs` 的
`operation_lock` / advisory lock / 恢复取消，都是典型的高并发正确性场景，但：

- 压测 server 未启用 `admin` / `backup` feature；
- e2e 也没有对应场景；
- 现有 workflow 只跑单实例。

### 4.8 判定侧还会「弱化」信号

- `run.exclude` 剔除了断言型场景，压测中剩下的断言也更少。
- `max_failure_rate=1%` 允许少量 5xx 不失败。
- `load_compare.py` 不展示 `errors`（按错误 kind 的明细），也不把 Prometheus 指标纳入判定。
- 基线差异只标注，不阻断。

## 5. 能压出来的问题（现有能力）

以下问题现有压测**能**发现，不需要改造：

- 接口 5xx / 409 / 唯一约束冲突（如果发生在 `run` 路径）。
- 客户端 5s 超时、服务端处理不过来。
- 上传信号量耗尽导致的 503（`upload_semaphore`，按 CPU 核数 ×2）。
- 密码校验信号量排队（`password_verify_semaphore`）。
- DB 连接池耗尽导致的超时（`max_connections` 默认 64，指标已采集）。
- 吞吐下降、P95 恶化（与基线对比）。
- 资源泄漏：`teardown` 失败率、S3 对象未删除、Redis key 未清理。

## 6. 改造方案

### P0：先让「静默竞态」变成可判定

1. **引入聚合不变量对账**
   - 每轮 `validate` 或场景结束时，直查 DB + 调接口对比：
     - `visual.like_count == count(visual_like where visual_id=?)`
     - `visual.comment_count == count(comment where visual_id=?)`
     - `comment.like_count == count(comment_like where comment_id=?)`
     - `collection.visual_count == count(collection_visual where collection_id=?)`
     - 时间线统计合计 == 该用户影像总数
   - 对账失败单独计为 `invariant_failures`，不要混进 `validate_failures`，避免与业务断言混淆。

2. **新增共享热点场景**
   - 同一 visual：N 用户并发 like/unlike 循环，结束后计数归零、无重复行。
   - 同一 comment：N 用户并发点赞/取消，结束后 `comment.like_count` 与行数一致。
   - 同一 collection：同一用户并发 add 同一批 visual，`visual_count` 只增加一次。
   - 同一内容并发上传：只允许 1 条记录，其余返回 400（不是 500）。
   - 同账号并发：登录、刷新、改密、换头像、登出（需要允许 `task_index` 复用账号）。

3. **压测报告增强**
   - 展示 `errors` 明细（按错误 kind/状态码）。
   - 把 `prometheus.txt` 中的 `database.connections.active/idle/max`、缓存命中率、限流 503 纳入报告。
   - 对账失败单独列表，不拉低「有效通过率」而是单独判失败。

4. **缓存两种模式对比**
   - `MEMORY_SEEK_CACHE__ENABLED=true/false` 各跑一遍，区分「DB 竞态」与「缓存竞态」。
   - `example.config.yaml:35-45` 已支持该开关。

### P1：按模块补对抗型场景

| 模块 | 新增场景 |
| --- | --- |
| auth | 同账号并发登录（校验最终 refresh_token 与 Redis access_token 一致）、同 token 并发刷新、同邮箱并发发码（验证码覆盖/限流） |
| user | 同账号并发改昵称（最终值 = 最后一次成功写）、并发换头像（所有成功响应的 token 都必须可下载）、并发改密（只有一个成功） |
| visual | 同内容并发上传、同影像并发删除+读取、删除与列表/详情并发、跨月首次上传 |
| collection | 同收藏夹并发 add/remove、并发删除+读取 |
| admin | 多实例领取同一任务（只允许一个 running）、取消与完成竞争、心跳失联回收、`run_id` fencing 覆盖 |
| backup | 并发手动备份/恢复（operation_lock）、恢复中取消、多实例 advisory lock |
| system | 指标数值对账（如 like 次数与 DB 行数） |

### P2：压测框架能力

1. **混合负载**：`memseek-test` 目前场景串行，需要一个场景内混合多个请求类型（读/写按比例）。
2. **阶梯加压**：并发 8 → 32 → 64 → 128，观察拐点而不是固定 32。
3. **多实例**：至少 2 个 server 实例共享 PG/Redis，才能暴露 L1 缓存与 admin fencing 问题。
4. **优雅关闭**：压测中向 server 发 SIGINT，验证「完成当前轮次、回到 queued、无半套数据」。
5. **多进程压测端**：单进程 32 并发受客户端 runtime 限制，高并发需多进程/多机。
6. **错误分类**：把 5xx、409、400、超时分开统计，避免业务错误掩盖真实故障。

## 7. 单场景压测策略：同一影像并发点赞/取消

这是「对抗型并发」的模板场景：一个共享资源、三类角色、四条不变量。
其他场景（评论、收藏夹、上传、同账号写）按同样结构套用即可。

### 7.1 为什么选这个场景

- 同时覆盖四类机制：`visual_like` 唯一约束、`visual.like_count` 原子增量、
  点赞状态缓存、详情读路径。
- 现有 `LikeVisualScenario` 已经让 32 个不同用户点赞同一张
  `seed_file_2_1`，但 `validate` 只查自己那一行（`like.rs:106`），
  改造代价最小、复用现有种子。
- 没有上传信号量、视频转码等噪音，失败信号干净。

### 7.2 目标不变量

| 编号 | 不变量 | 检查方式 |
| --- | --- | --- |
| I1 | 同一 `(user_id, visual_id)` 至多一条点赞记录 | `GROUP BY ... HAVING count(*) > 1` 为空 |
| I2 | `visual.like_count == count(visual_like where visual_id=?)` | 直查 DB 对账 |
| I3 | 静默后计数归零 | 所有任务 teardown 后 `like_count=0 且行数=0` |
| I4 | 接口返回的 `like_count` 与 DB 一致 | 静默后 `GET /visual/visual/{id}` 对比 DB |
| I5 | 无 5xx；重复点赞只返回 400 | 状态码直方图 |
| I6 | 无负数计数、无 `duplicate key` / `deadlock detected` 日志 | `server.log` 扫描 |
| I7 | 缓存失效失败数为 0 | `cache_invalidate` 指标 / 日志 |

### 7.3 负载设计

总并发 32，按 `task.index` 分三类角色：

| 角色 | 任务数 | 账号 | 行为 | 目的 |
| --- | ---: | --- | --- | --- |
| writer | 20 | 20 个独立 `loadtest_hot_*` | 对同一 visual 循环 `like → unlike`，无 think time | 打同一行 `like_count` 的行锁，验证增量不丢、不死锁 |
| racer | 4 | **共享 1 个账号与 token** | 同时 `like / unlike` 同一 `(user, visual)` | 制造唯一约束竞争，验证「一成功多 400、无 500」 |
| reader | 8 | 8 个独立账号 | `GET /visual/visual/{id}` + `GET /visual` 列表 | 读写交错，验证读路径与缓存 |

数据准备：

- 专用热点影像 `seed_file_hotspot_1`，不要复用其它场景的 ordinal。
- 专用账号池 `loadtest_hot_1..20` + `loadtest_hot_shared`，避免与现有场景串号。
- **共享账号只登录一次**，把同一个 token 注入 4 个 racer 客户端。
  如果让 4 个 racer 各自登录，会先触发「同账号登录 token 覆盖」，
  把唯一约束竞争和登录竞争混在一起，无法定位。
- 开跑前删除相关缓存 key（`visual:{id}:info`、用户点赞状态 key），
  保证从冷缓存开始。

### 7.4 阶段编排

| 阶段 | 时长 | 并发 | 内容 | 观察点 |
| --- | ---: | ---: | --- | --- |
| P0 冷启动 | 10s | 8 | 1 writer + 7 reader | 空缓存下首次详情读、首次计数更新 |
| P1 阶梯 | 4 × 15s | 8/16/32/64 | writer + reader | 行锁排队、P95 拐点、连接池峰值 |
| P2 稳态 | 120s | 32 | 20 writer + 4 racer + 8 reader | 核心不变量与错误语义 |
| P3 静默结算 | 10s | 0 | 停止流量，等待 in-flight 结束 | I1–I4、I6 |
| P4 关缓存对照 | 120s | 32 | 同 P2，server 加 `MEMORY_SEEK_CACHE__ENABLED=false` | 区分 DB 竞态与缓存竞态 |
| P5 多实例（可选） | 120s | 32 | 2 个 server 实例共享 PG/Redis | L1 陈旧、跨实例失效 |

当前框架里，阶段通过**多次独立运行**编排（`run.scenarios` 白名单 +
`E2E__RUN__CONCURRENCY` 覆盖）；角色在同一个场景内用 `task.index` 分支实现。

### 7.5 断言分层

| 层级 | 时机 | 断言 | 失败归属 |
| --- | --- | --- | --- |
| 轻量 | 每轮 `run`/`validate` | 状态码 ∈ 预期；自己的点赞行存在/不存在 | `failures` / `validate_failures` |
| 采样 | 每 N 轮 reader | 接口 `like_count` ∈ `[0, 当前 writer 数]`，不为负 | `validate_failures` |
| 结算 | P3 之后 | I1、I2、I3、I4、I6 | **`invariant_failures`**（新增） |
| 日志 | 运行结束 | 扫描 `duplicate key`、`deadlock detected`、`cache_invalidate` 错误 | 单独列表 |

注意：稳态期间**不要**断言接口计数与 DB 严格相等。单实例下也存在
「写者已提交、缓存尚未失效」的毫秒级窗口；强一致断言只放在静默结算阶段。

### 7.6 判定阈值

正确性模式（P2/P3）：

- `invariant_failures = 0`；
- 5xx = 0；
- `duplicate key` / `deadlock detected` = 0；
- 400 只允许出现在 racer 的预期冲突中，且 writer 不应出现 400。

负载模式（P1）：

- 5xx ≤ 0.1%，超时率 ≤ 0.5%；
- P95 不超过基线 1.2 倍，RPS 不低于基线 0.9 倍（仅标注，与现有口径一致）；
- `database.connections.active` 峰值不超过 `max` 的 90%，否则视为连接池配置问题。

### 7.7 观测指标

- 客户端：RPS、P50/P95/P99、状态码直方图、按错误 kind 计数。
- 服务端：`server_http_requests`、`visual:like:*` 的 attempts/success/duration、
  `database.connections.active/idle/max`、缓存 `l1/l2` 命中率与 `cache_invalidate` 失败数。
- PostgreSQL：`pg_stat_database.deadlocks`、`pg_stat_activity` 的锁等待（可做快照）。
- 日志：`duplicate key`、`deadlock detected`、`pool timed out`。

### 7.8 需要补的框架能力

| 能力 | 现状 | 改动 |
| --- | --- | --- |
| 角色分流 | 可用 `task.index` 在 `run` 里分支 | 无需改框架 |
| 共享 token 的 racer | `Session` 每任务独立登录 | 在 `Context` 预置共享 `Session`，或在 setup 里只登录一次并缓存 |
| 全局结算对账 | 无场景结束后钩子 | `main.rs` 在 `manager.run` 之后跑 `reconcile`，或加 `--reconcile` |
| 阶段编排 | 单次运行固定并发/时长 | 用多次运行 + 白名单，或给 `memseek-test` 加 ramp |
| 混合负载 | 一个场景一个请求类型 | 用 `task.index` 角色分流即可 |
| 错误分类 | 只有 `errors` map | 报告按 5xx/4xx/超时拆分，并新增 `invariant_failures` |
| 同步起跑（可选） | 无 barrier | `Context` 放 `Arc<Barrier>`，racer 的 `run` 前等待 |

### 7.9 运行方式

```bash
# P2 稳态：只跑热点场景，32 并发，120s
E2E__RUN__SCENARIOS=HotLikeConcurrencyScenario \
E2E__RUN__MODE=duration \
E2E__RUN__DURATION_SECS=120 \
E2E__RUN__CONCURRENCY=32 \
E2E__RUN__EXCLUDE="" \
cargo run -p e2e --release -- -c tests/e2e/e2e.config.yml \
  --report-json hot-like.json

# P1 阶梯：并发 8/16/32/64 各跑一次，汇总 P95 与错误率
for c in 8 16 32 64; do
  E2E__RUN__SCENARIOS=HotLikeConcurrencyScenario \
  E2E__RUN__MODE=duration E2E__RUN__DURATION_SECS=15 \
  E2E__RUN__CONCURRENCY=$c \
  cargo run -p e2e --release -- -c tests/e2e/e2e.config.yml \
    --report-json "hot-like-$c.json"
done

# 判定
python3 tests/e2e/ci/load_compare.py --current hot-like.json --baseline <基线>
```

### 7.10 这套策略能发现 / 发现不了什么

能发现：

- 计数丢失更新（如果增量改成「读-改-写」）、计数变负；
- 唯一约束竞争导致的 500；
- 行锁竞争导致的 P95 抬升、死锁、连接池耗尽；
- L2 旧值回填（P4 关缓存对照 + 结算读）；
- 多实例 L1 陈旧（P5，需要 2 实例）。

发现不了：

- 其它实例 L1 的 60s 窗口（必须 P5 多实例）；
- S3 对象泄漏（本场景不碰对象存储）；
- 评论/收藏夹/上传的计数问题（需要按同一模板另建场景，见 P1）。

### 7.11 同一模板迁移到「同内容并发上传」

- 角色：4 个 writer 共享同一份字节（同一个 BLAKE3），其余 reader 只读。
- 并发必须 ≤ 上传信号量许可数（默认 CPU 核数 ×2），否则大量 503 会掩盖竞态；
  CI 2 核 = 4 个许可，正好 4 并发。
- 判定语义要分开：**400 = 正常去重，503 = 过载，500 = 竞态 bug**。
- 静默后不变量：该 hash 在 `visual_visual` 恰好 1 行；S3 对象恰好 1 份；
  无 `duplicate key value violates unique constraint` 的 500。
- 当前代码 `exists_by_hash` → `insert_visual` 之间没有原子保护
  （`upload.rs:188`、`visual_repo.rs:194`），4 个同步请求就可能复现。

## 8. 验收标准示例（DoD）

- 同一 visual 上 32 用户并发 like/unlike 60s：
  - 无 5xx；
  - 结束后 `visual.like_count == count(visual_like)`；
  - 同一 `(user_id, visual_id)` 无重复行。
- 同一内容并发上传 32 次：
  - 恰好 1 个 2xx，其余为 400（去重），**不允许 500**；
  - `visual_visual.hash` 唯一约束无冲突日志。
- 同账号并发改密 2 次（不同新密码）：
  - 恰好 1 个成功；
  - 成功密码可登录，失败密码不可登录。
- 同账号并发换头像 2 次：
  - 两个成功响应返回的 token 均可下载到对应内容；
  - DB 只指向其中一个 key，另一个对象允许被回收。
- 多实例（2 实例）下修改昵称后立即读：
  - 另一实例在 `local_ttl_secs` 内返回新值，或明确接受 60s 最终一致（需要产品确认）。
- admin 双实例并发领取：
  - 同一 `job_id` 只有一个实例进入 `running`；
  - 结果写入以 `run_id` fencing，旧执行者不能覆盖新结果。

## 9. 风险与取舍

- **加聚合对账会引入额外 DB 查询**：建议只在场景结束/固定周期做，不要每轮全量扫。
- **同账号并发会破坏现有「账号池隔离」假设**：需要新增专用账号池或在场景内显式复用同一账号。
- **多实例压测成本更高**：CI runner 需要起 2 个 server 进程，资源和端口都要重新编排。
- **缓存一致性需要产品口径**：60s L1 陈旧是否可接受，决定对账是「强一致」还是「最终一致」。
- **部分竞态窗口极窄**：如跨月首次上传、上传同内容，需要刻意构造并发和时序，不能靠现有随机流量碰运气。

## 9. 落地状态（2026-11）

- [x] `memseek-test 0.26.0`：`Scenario::reconcile`、`status_code`、`invariant_failures`、`status_counts`
- [x] `tests/e2e`：Verdict/JSON/`load_compare.py` 接入对账与 4xx/5xx
- [x] 共享热点场景 `HotLikeConcurrencyScenario`（同影像并发点赞 + 计数对账）
- [x] `Context` 共享 Session / Barrier / 计数器
- [ ] 阶梯加压（ramp）
- [ ] 多实例 L1 缓存一致性验证
- [ ] 同内容并发上传 / 同账号写竞争场景
