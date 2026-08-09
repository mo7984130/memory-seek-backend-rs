# Dashboard 通用设计规范

## 概述

本规范定义了 Grafana Dashboard 的统一设计标准，确保各模块（auth、user、photo）的监控面板风格一致、布局清晰。

## 命名规范

### 指标命名

- 使用冒号 `:` 分隔层级：`{crate}:{func}:{step}`
- 计时指标为原生 histogram，Prometheus 导出为 `{name}_bucket` / `{name}_sum` / `{name}_count`
- 示例：`auth:login:attempts`、`auth:login:duration_seconds`、`user:get_user_info:db_query`
- 系统指标使用点号层级，导出后变为下划线（`system.cpu.usage` → `system_cpu_usage`）

完整清单见 [metrics-naming.md](./metrics-naming.md)。

### Row 标题

格式：`<中文名> (<英文标识>)`

```
用户登录 (login)
获取用户信息 (get_user_info)
上传照片 (upload_photo)
```

### Panel 标题

格式：`<中文操作名><指标类型>`（无空格、无"与"）

```
登录耗时
登录子步骤耗时
登录调用量成功率
```

---

## 面板布局

### 每个操作的标准面板（3 个）

#### 单行布局：6w + 6w + 12w（三个面板在同一行）

| 面板 | 类型 | gridPos | 说明 |
|------|------|---------|------|
| `<操作名>耗时` | timeseries (line) | `x: 0, w: 6` | P50 / P95 / P99 三条折线 |
| `<操作名>子步骤耗时` | timeseries (filled line) | `x: 6, w: 6` | 各子步骤平均耗时，填充面积 |
| `<操作名>调用量成功率` | timeseries (组合图) | `x: 12, w: 12` | 左轴 QPS，右轴成功率 |

三个面板共享同一个 `y` 值，`h` 均为 8。下一个操作的 row 位于 `y + 9`。

#### HTTP 请求汇总 row（每个 Dashboard 顶部）

所有业务 dashboard 顶部统一放置 `HTTP 请求 (http)` row，三面板等宽（8w + 8w + 8w）：

| 面板 | 类型 | 查询 |
|------|------|------|
| `HTTP QPS` | timeseries | `sum(rate(server_http_requests[5m])) by (route)` |
| `HTTP 错误率` | timeseries | `sum(rate(server_http_requests{status_class="5xx"}[5m])) / sum(rate(server_http_requests[5m])) * 100` |
| `HTTP 延迟 (P50/P95/P99)` | timeseries | `histogram_quantile(0.95, sum(rate(server_http_duration_seconds_bucket[5m])) by (le, route)) * 1000` |

> counter 不带 `_total` 后缀（metrics-exporter-prometheus 不追加），点号转下划线。

#### 错误分布面板（埋了 `errors:{kind}` 的操作）

操作若存在 `{crate}:{func}:errors:*` 指标，追加第四个面板（可选，与前三面板同行下方或并排）：

```json
{
  "targets": [
    { "expr": "sum(rate(<metric>:errors:*[5m])) by (kind)", "legendFormat": "{{kind}}" }
  ]
}
```

---

## Panel 配置

> 耗时指标的原始单位为**秒**（`MetricsTimer` 记录 `as_secs_f64`），
> 面板以 `ms` 为单位展示，因此所有耗时表达式末尾追加 `* 1000`。

### 耗时面板 (timeseries)

```json
{
  "type": "timeseries",
  "gridPos": { "h": 8, "w": 6 },
  "fieldConfig": {
    "defaults": {
      "unit": "ms",
      "custom": {
        "drawStyle": "line",
        "lineInterpolation": "smooth",
        "lineWidth": 2,
        "fillOpacity": 10
      },
      "thresholds": {
        "steps": [
          { "color": "green", "value": null },
          { "color": "yellow", "value": 100 },
          { "color": "red", "value": 500 }
        ]
      }
    }
  },
  "targets": [
    { "expr": "histogram_quantile(0.5, sum(rate(<metric>:duration_seconds_bucket[5m])) by (le)) * 1000", "legendFormat": "P50" },
    { "expr": "histogram_quantile(0.95, sum(rate(<metric>:duration_seconds_bucket[5m])) by (le)) * 1000", "legendFormat": "P95" },
    { "expr": "histogram_quantile(0.99, sum(rate(<metric>:duration_seconds_bucket[5m])) by (le)) * 1000", "legendFormat": "P99" }
  ]
}
```

### 子步骤耗时面板 (timeseries - filled)

子步骤耗时有两类命名：

1. `.timed(metrics_name!("step"))` 产生的 histogram：`{crate}:{func}:{step}`，查询 `<metric>:<step>_sum/_count`
2. `timed!("step", ...)` 产生的 histogram：`{crate}:{func}:{step}:duration_seconds`，查询 `<metric>:<step>:duration_seconds_sum/_count`

```json
{
  "type": "timeseries",
  "gridPos": { "h": 8, "w": 6 },
  "fieldConfig": {
    "defaults": {
      "unit": "ms",
      "custom": {
        "drawStyle": "line",
        "lineInterpolation": "smooth",
        "lineWidth": 1,
        "fillOpacity": 40,
        "showPoints": "never"
      }
    }
  },
  "targets": [
    { "expr": "rate(<metric>:<step>_sum[5m]) / rate(<metric>:<step>_count[5m]) * 1000", "legendFormat": "<步骤中文名>" }
  ]
}
```

### 调用量成功率组合图 (timeseries)

```json
{
  "type": "timeseries",
  "gridPos": { "h": 8, "w": 12, "x": 12 },
  "fieldConfig": {
    "defaults": {
      "unit": "reqps",
      "custom": {
        "drawStyle": "line",
        "lineWidth": 2
      },
      "thresholds": {
        "mode": "absolute",
        "steps": [{ "color": "green", "value": null }]
      }
    },
    "overrides": [
      {
        "matcher": { "id": "byName", "options": "成功率" },
        "properties": [
          { "id": "custom.axisPlacement", "value": "right" },
          { "id": "custom.axisLabel", "value": "成功率" },
          { "id": "unit", "value": "percent" },
          { "id": "min", "value": 0 },
          { "id": "thresholds", "value": {
            "mode": "absolute",
            "steps": [
              { "color": "red", "value": null },
              { "color": "yellow", "value": 95 },
              { "color": "green", "value": 99 }
            ]
          }}
        ]
      }
    ]
  },
  "targets": [
    { "expr": "rate(<metric>:attempts[5m])", "legendFormat": "QPS" },
    { "expr": "rate(<metric>:success[5m]) / rate(<metric>:attempts[5m]) * 100", "legendFormat": "成功率" }
  ]
}
```

---

## 单位规范

| 指标类型 | 单位 | 说明 |
|----------|------|------|
| 耗时 | `ms` | 原始值（秒）经 `* 1000` 换算 |
| 子步骤耗时 | `ms` | 原始值（秒）经 `* 1000` 换算 |
| QPS | `reqps` | 每秒请求数 |
| 成功率 | `percent` | 百分比（不设上限） |

---

## 阈值标准

| 指标 | 绿色 | 黄色 | 红色 | 配置位置 |
|------|------|------|------|----------|
| 耗时 (ms) | < 100 | 100 - 500 | > 500 | defaults.thresholds |
| 成功率 (%) | > 99 | 95 - 99 | < 95 | overrides（成功率 series） |
| QPS | 无阈值 | - | - | defaults.thresholds 仅 `{ "color": "green", "value": null }` |

> 注意：成功率阈值配置在 override 的 `thresholds` 属性中（针对"成功率"series），而非 defaults 中。defaults 的 thresholds 仅设置一个无阈值的绿色步骤。

---

## 时间与刷新配置

```json
{
  "time": {
    "from": "now-5m",
    "to": "now"
  },
  "refresh": "5s"
}
```

| 配置项 | 值 | 说明 |
|--------|-----|------|
| 默认时间范围 | `now-5m` ~ `now` | 最近 5 分钟 |
| 自动刷新间隔 | `5s` | 每 5 秒刷新一次 |

---

## 选项配置

### Legend

```json
{
  "legend": {
    "calcs": ["lastNotNull"],
    "displayMode": "list",
    "placement": "bottom"
  }
}
```

### Tooltip

```json
{
  "tooltip": {
    "mode": "single",
    "sort": "none"
  }
}
```

---

## 示例结构

```
Auth 模块监控
├── 用户登录 (login)                    ← row
│   ├── 登录耗时                    [x:0,  w:6]  ← 同一行 (y=N)
│   ├── 登录子步骤耗时              [x:6,  w:6]  ← 同一行 (y=N)
│   └── 登录调用量成功率            [x:12, w:12] ← 同一行 (y=N)
├── 用户注册 (register)                 ← row (y=N+9)
│   ├── 注册耗时                    [x:0,  w:6]
│   ├── 注册子步骤耗时              [x:6,  w:6]
│   └── 注册调用量成功率            [x:12, w:12]
├── 发送邮箱验证码 (send_email_code)    ← row
│   ├── ...
```

---

## 更新记录

- 2026-06-18: 初始版本，统一 auth/user/photo 三个模块的 dashboard 设计
- 2026-06-19: 默认时间范围改为 5 分钟，刷新间隔改为 5 秒
- 2026-06-19: 移除并发度相关设计（底部汇总区、并发度 target/override、单位/阈值定义）
- 2026-06-19: 明确单行布局（6+6+12 同一 y 值），Panel 标题去掉"与"，成功率阈值移至 override
- 2026-08-01: 指标命名改为 `{crate}:{func}:{step}`，耗时改用原生 histogram
  （`histogram_quantile` + `_bucket` / 平均耗时 `_sum / _count`），耗时单位换算 `* 1000`
- 2026-08-09: 新增 `HTTP 请求 (http)` 汇总 row（QPS / 错误率 / 延迟）置于各 dashboard 顶部；
  新增错误分布面板规范；system.json 重构为 HTTP / CPU / 内存 / 磁盘 / 数据库 / Redis / 版本
  布局并增补 `system_cpu_cores` / `system_disk_*` / `database_connections_max` / `server_build_info`
  面板。
