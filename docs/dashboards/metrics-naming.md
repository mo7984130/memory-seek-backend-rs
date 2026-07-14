# Metrics 指标命名规范

## 命名格式

指标名使用冒号 `:` 分隔，格式为：

```
module:operation:target:metric_type
```

## 计时指标命名

### 核心指标

| 指标名 | 含义 |
|--------|------|
| `{name}:duration_quantile` | 预计算的 summary 分位数（带 quantile 标签），**不是** histogram 原始分桶数据 |
| `{name}:duration_quantile_count` | 请求计数 |
| `{name}:duration_quantile_sum` | 总耗时累加 |

### 示例

```
user:get_user_info:duration_quantile          # 用户获取信息的分位数统计
auth:login:db_query:duration_quantile_count   # 登录数据库查询的请求计数
auth:login:db_query:duration_quantile_sum     # 登录数据库查询的总耗时
```

## 注意事项

1. **不要使用** `duration_bucket` — 这是 histogram 的原始桶数据，我们使用的是 summary 预计算分位数
2. **quantile 标签** — 实际查询时需要带 `quantile="0.5"` / `quantile="0.95"` 等标签来获取具体分位数
3. **分隔符统一** — 所有指标名层级使用英文冒号 `:` 分隔，不使用下划线或点号
