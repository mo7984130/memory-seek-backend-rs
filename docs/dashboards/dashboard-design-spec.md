# Dashboard 通用设计规范

## 概述

本规范定义了 Grafana Dashboard 的统一设计标准，确保各模块（auth、user、photo）的监控面板风格一致、布局清晰。

## 命名规范

### 指标命名

- 使用冒号 `:` 分隔层级：`module:operation:target:metric_type`
- 计时指标使用 `duration_quantile`（summary 预计算分位数）
- 示例：`auth:login:duration_quantile`、`user:get_user_info:db_query:duration_quantile`

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

---

## Panel 配置

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
    { "expr": "<metric>:duration_quantile{quantile=\"0.5\"}", "legendFormat": "P50" },
    { "expr": "<metric>:duration_quantile{quantile=\"0.95\"}", "legendFormat": "P95" },
    { "expr": "<metric>:duration_quantile{quantile=\"0.99\"}", "legendFormat": "P99" }
  ]
}
```

### 子步骤耗时面板 (timeseries - filled)

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
    { "expr": "rate(<metric>:<step>:duration_quantile_sum[5m]) / rate(<metric>:<step>:duration_quantile_count[5m])", "legendFormat": "<步骤中文名>" }
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
| 耗时 | `ms` | 毫秒 |
| 子步骤耗时 | `ms` | 毫秒 |
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
