# admin 域

管理端聚合层：提供 `/admin/*` 路由，并把后台任务的排队、领取、取消与审计落到数据库。

## 多实例模型

数据库是任务状态的唯一事实源，实例只持有执行句柄：

| 关注点 | 机制 |
|---|---|
| 同类型任务去重 | 部分唯一索引 `uniq_admin_job_active_kind`（`status IN queued/running/cancel_requested`） |
| 任务领取 | `SELECT ... FOR UPDATE SKIP LOCKED` + 更新为 `running` |
| 执行者存活 | `heartbeat_at`，默认 10s 心跳、30s 判定失联 |
| 结果防覆盖 | 每次领取生成 `run_id`，完成写入以 `run_id` 为 fencing token |
| 跨实例取消 | 取消写入 `cancel_requested`，执行实例在心跳周期读取并取消本地执行 |
| 实例崩溃 | 心跳停止 → 任意存活实例回收为 `failed(instance_lost)`，等待人工重试 |
| 任务卡死但实例存活 | 管理员调用 `POST /admin/jobs/{id}/abandon` 强制标记失败 |
| 优雅关闭 | 执行器在宽限期内退出则回到 `queued`，其它实例立即接手 |

### 状态机

```
                claim
  queued ───────────────► running ──成功──► succeeded
    │                       │
    │ cancel                ├──失败──► failed
    ▼                       ├──取消确认──► cancelled
 cancelled                  ├──心跳中断──► failed(instance_lost)
                            └──人工放弃──► failed
```

- 自动 fail 只负责释放任务，不会自动重跑；失败后需要重新调用 `POST /admin/jobs`。
- `FaceFull` 运行中不可取消（已清空旧数据，中断无法安全回滚），但可以人工放弃。
- 备份恢复 `backup_restore` 同样走任务队列；取消会在每张表导入前与提交前生效，事务回滚，不会留下半套数据。
- 任务参数（二次聚类 `threshold`、恢复请求等）持久化在 `payload`，重启不丢。
- 心跳与回收全部使用数据库时间，避免实例间时钟偏差。

## 模块划分

```
tasks/
├── mod.rs         对外导出
├── config.rs      轮询 / 心跳 / 失联阈值 / 并发配置
├── repo.rs        唯一写 SQL 的地方
├── service.rs     HTTP 用例、事务、审计
├── dispatcher.rs  任务类型 → visual / backup 执行器
├── runner.rs      单任务执行生命周期
└── supervisor.rs  每实例常驻的领取与回收循环
```

## 配置

```yaml
admin:
  poll_interval_secs: 5
  heartbeat_interval_secs: 10
  stale_after_secs: 30
  max_concurrency: 2
```

`heartbeat_interval_secs` 会被限制为不超过 `stale_after_secs` 的一半。

## 部署

旧版本启动时会把所有活动任务改写为 `interrupted`，**不能与新版本混跑**。升级步骤：

1. 停止全部旧实例；
2. 删除旧任务表（旧数据不保留，`schema-sync` 只增不删，必须手工删除）：
   ```sql
   DROP TABLE IF EXISTS admin_job;
   ```
3. 启动新实例，`types-db-init` 会自动重建表并创建唯一部分索引。

备份定时器在每个实例都会注册，但通过 PostgreSQL advisory lock 保证同一时刻只有一个实例执行。
