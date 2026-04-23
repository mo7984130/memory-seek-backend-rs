# Grafana Dashboard 修改计划

## 概述

根据 Grafana Dashboard Metrics 标准化规范,对 `doc/stress-test/dashboard.json` 进行全面修改,确保所有耗时拆解使用 `irate + [1m]` 模式,并为每个方法添加完整的监控面板(统计、耗时拆解、P99)。

## 修改原则

### 1. 耗时拆解查询优化

**当前问题**: 使用 `sum/count` 计算平均值,无法反映实时性能变化

**修改方案**: 使用 `irate(metric[1m])` 模式

**对比示例**:
```promql
# 修改前
login_verify_seconds_sum / login_verify_seconds_count

# 修改后
irate(login_verify_seconds_sum[1m]) / irate(login_verify_seconds_count[1m])
```

### 2. 每个方法的完整监控体系

每个方法需要三个核心面板:

1. **统计面板** - 显示尝试次数和成功次数
2. **耗时拆解面板** - 显示各个环节的耗时(使用 irate)
3. **P99 延迟面板** - 显示 99 分位延迟

## 详细修改清单

### 一、耗时拆解面板修改

#### 1. 登录相关耗时拆解

**面板 ID: 11 - 登录各环节平均耗时**
- 位置: 认证监控区域
- 需要修改的查询:
  - `login_verify_seconds_sum/count` → `irate(...[1m])`
  - `login_db_query_duration_seconds_sum/count` → `irate(...[1m])`
  - `login_refresh_token_cost_seconds_sum/count` → `irate(...[1m])`
  - `login_redis_seconds_sum/count` → `irate(...[1m])`

**面板 ID: 22 - 登录耗时拆解 (堆叠)**
- 位置: 性能分析区域
- 需要修改的查询:
  - 所有上述查询 + `login_crypto_avatar_token_seconds_sum/count`

#### 2. 注册相关耗时拆解

**面板 ID: 23 - 注册耗时拆解**
- 位置: 性能分析区域
- 需要修改的查询:
  - `register_total_seconds_sum/count` → `irate(...[1m])`
  - `verify_email_code_seconds_sum/count` → `irate(...[1m])`
  - `verify_inviter_code_seconds_sum/count` → `irate(...[1m])`
  - `hash_password_seconds_sum/count` → `irate(...[1m])`
  - `db_insert_seconds_sum/count` → `irate(...[1m])`

#### 3. Token 刷新相关耗时拆解

**面板 ID: 24 - Token刷新耗时拆解**
- 位置: 性能分析区域
- 需要修改的查询:
  - `refresh_access_token_total_seconds_sum/count` → `irate(...[1m])`
  - `verify_refresh_token_seconds_sum/count` → `irate(...[1m])`
  - `set_access_token_seconds_sum/count` → `irate(...[1m])`

#### 4. 验证码发送相关耗时拆解

**面板 ID: 25 - 验证码发送耗时拆解**
- 位置: 性能分析区域
- 需要修改的查询:
  - `send_email_code_total_seconds_sum/count` → `irate(...[1m])`
  - `redis_set_seconds_sum/count` → `irate(...[1m])`

#### 5. 用户模块相关耗时拆解

**面板 ID: 31 - 用户信息查询耗时拆解**
- 位置: 用户模块监控区域
- 需要修改的查询:
  - `get_user_info_db_query_seconds_sum/count` → `irate(...[1m])`
  - `get_user_info_encrypt_avatar_seconds_sum/count` → `irate(...[1m])`

**面板 ID: 34 - 修改昵称耗时拆解**
- 位置: 用户模块监控区域
- 需要修改的查询:
  - `change_nickname_db_update_seconds_sum/count` → `irate(...[1m])`
  - `change_nickname_redis_delete_seconds_sum/count` → `irate(...[1m])`

### 二、P99 延迟面板添加

需要为以下方法添加 P99 延迟监控面板:

#### 1. 认证模块

**登录 P99 延迟**
- 面板标题: "登录 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(login_duration_seconds_bucket[1m]))) * 1000`
- 位置: 认证监控区域,登录并发数面板下方
- GridPos: x=0, y=26, w=8, h=8

**注册 P99 延迟**
- 面板标题: "注册 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(register_duration_seconds_bucket[1m]))) * 1000`
- 位置: 认证监控区域
- GridPos: x=8, y=26, w=8, h=8

**Token 刷新 P99 延迟**
- 面板标题: "Token刷新 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(refresh_token_duration_seconds_bucket[1m]))) * 1000`
- 位置: 认证监控区域
- GridPos: x=16, y=26, w=8, h=8

**验证码发送 P99 延迟**
- 面板标题: "验证码发送 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(send_email_code_duration_seconds_bucket[1m]))) * 1000`
- 位置: 认证监控区域
- GridPos: x=0, y=34, w=8, h=8

#### 2. 用户模块

**用户信息查询 P99 延迟**
- 面板标题: "用户信息查询 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(get_user_info_duration_seconds_bucket[1m]))) * 1000`
- 位置: 用户模块监控区域
- GridPos: 需要调整现有面板位置

**修改昵称 P99 延迟**
- 面板标题: "修改昵称 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(change_nickname_duration_seconds_bucket[1m]))) * 1000`
- 位置: 用户模块监控区域

**修改密码 P99 延迟**
- 面板标题: "修改密码 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(change_password_duration_seconds_bucket[1m]))) * 1000`
- 位置: 用户模块监控区域

**登出 P99 延迟**
- 面板标题: "登出 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(logout_duration_seconds_bucket[1m]))) * 1000`
- 位置: 用户模块监控区域

**批量获取用户信息 P99 延迟**
- 面板标题: "批量获取用户信息 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(get_user_info_batch_duration_seconds_bucket[1m]))) * 1000`
- 位置: 用户模块监控区域

**邀请码生成 P99 延迟**
- 面板标题: "邀请码生成 P99延迟"
- 查询: `histogram_quantile(0.99, sum by (le) (irate(generate_inviter_code_duration_seconds_bucket[1m]))) * 1000`
- 位置: 用户模块监控区域

## 面板布局调整

### 认证监控区域 (y=17 开始)

```
Row: 认证监控 (y=17, h=1)
├─ 登录统计 (x=0, y=18, w=8, h=8)
├─ 登录各环节平均耗时 (x=8, y=18, w=8, h=8) [修改]
└─ 登录并发数 (x=16, y=18, w=8, h=8)

├─ 注册统计 (x=0, y=26, w=8, h=8)
├─ Token 刷新统计 (x=8, y=26, w=8, h=8)
└─ 验证码发送统计 (x=16, y=26, w=8, h=8)

新增行:
├─ 登录 P99延迟 (x=0, y=34, w=8, h=8) [新增]
├─ 注册 P99延迟 (x=8, y=34, w=8, h=8) [新增]
└─ Token刷新 P99延迟 (x=16, y=34, w=8, h=8) [新增]

├─ 验证码发送 P99延迟 (x=0, y=42, w=8, h=8) [新增]
```

### 用户模块监控区域 (y=34 开始,需要调整)

```
Row: 用户模块监控 (y=51, h=1) [调整位置]
├─ 用户信息查询统计 (x=0, y=52, w=8, h=8)
├─ 用户信息查询耗时拆解 (x=8, y=52, w=8, h=8) [修改]
└─ 邀请码生成统计 (x=16, y=52, w=8, h=8)

├─ 修改昵称统计 (x=0, y=60, w=8, h=8)
├─ 修改昵称耗时拆解 (x=8, y=60, w=8, h=8) [修改]
└─ 修改密码统计 (x=16, y=60, w=8, h=8)

新增行:
├─ 用户信息查询 P99延迟 (x=0, y=68, w=8, h=8) [新增]
├─ 修改昵称 P99延迟 (x=8, y=68, w=8, h=8) [新增]
└─ 修改密码 P99延迟 (x=16, y=68, w=8, h=8) [新增]

├─ 登出统计 (x=0, y=76, w=8, h=8)
├─ 批量获取用户信息统计 (x=8, y=76, w=8, h=8)
└─ 用户模块并发数 (x=16, y=76, w=8, h=8)

新增行:
├─ 登出 P99延迟 (x=0, y=84, w=8, h=8) [新增]
├─ 批量获取用户信息 P99延迟 (x=8, y=84, w=8, h=8) [新增]
└─ 邀请码生成 P99延迟 (x=16, y=84, w=8, h=8) [新增]
```

### 性能分析区域 (调整后)

```
Row: 性能分析 (y=92, h=1) [调整位置]
├─ 登录耗时拆解 (堆叠) (x=0, y=93, w=12, h=8) [修改]
└─ 注册耗时拆解 (x=12, y=93, w=12, h=8) [修改]

├─ Token刷新耗时拆解 (x=0, y=101, w=12, h=8) [修改]
└─ 验证码发送耗时拆解 (x=12, y=101, w=12, h=8) [修改]
```

### 并发监控区域 (调整后)

```
Row: 并发监控 (y=109, h=1) [调整位置]
├─ 所有操作并发数 (x=0, y=110, w=12, h=8)
└─ 操作成功率 (x=12, y=110, w=12, h=8)
```

## 修改步骤

### 步骤 1: 修改耗时拆解查询 (优先级: 高)

1. 修改面板 ID 11 (登录各环节平均耗时)
2. 修改面板 ID 22 (登录耗时拆解堆叠)
3. 修改面板 ID 23 (注册耗时拆解)
4. 修改面板 ID 24 (Token刷新耗时拆解)
5. 修改面板 ID 25 (验证码发送耗时拆解)
6. 修改面板 ID 31 (用户信息查询耗时拆解)
7. 修改面板 ID 34 (修改昵称耗时拆解)

### 步骤 2: 添加 P99 延迟面板 (优先级: 高)

1. 在认证监控区域添加 4 个 P99 面板
2. 在用户模块监控区域添加 6 个 P99 面板

### 步骤 3: 调整面板布局 (优先级: 中)

1. 重新计算所有面板的 GridPos
2. 确保面板之间没有重叠
3. 保持逻辑分组清晰

### 步骤 4: 验证和测试 (优先级: 中)

1. 验证 JSON 格式正确性
2. 检查所有查询语法
3. 确认面板 ID 唯一性
4. 测试 dashboard 导入

## 预期成果

### 修改后的效果

1. **实时性提升**: 所有耗时指标使用 `irate + [1m]`,能够快速反映最近的性能变化
2. **监控完整性**: 每个方法都有统计、耗时拆解、P99 三个维度的监控
3. **可视化优化**: 耗时拆解使用堆叠图,便于分析性能瓶颈
4. **布局合理**: 面板按功能模块分组,便于快速定位问题

### 统计数据

- 修改耗时拆解面板: 7 个
- 新增 P99 延迟面板: 10 个
- 调整面板位置: 约 30 个
- 总面板数: 从 28 个增加到 38 个

## 注意事项

1. **指标名称假设**: P99 面板使用的 histogram 指标名称需要根据实际代码中的指标名称调整
2. **面板 ID 分配**: 新增面板需要分配唯一的 ID (从 39 开始)
3. **GridPos 计算**: 需要精确计算每个面板的位置,避免重叠
4. **数据源 UID**: 确保所有面板使用正确的数据源 UID (afgkqlgg7670gb)
5. **单位设置**: P99 延迟使用毫秒 (ms) 单位,耗时拆解使用秒 (s) 单位

## 后续优化建议

1. 添加告警规则配置
2. 增加业务指标监控 (如转化率、留存率)
3. 添加自定义变量,支持动态筛选
4. 优化查询性能,减少 Prometheus 负载
5. 添加文档注释,说明每个面板的用途

## 参考文档

- [Grafana Dashboard Metrics Skill](/.trae/skills/grafana-dashboard-metrics/SKILL.md)
- [Prometheus Query Functions](https://prometheus.io/docs/prometheus/latest/querying/functions/)
- [Grafana Panel Configuration](https://grafana.com/docs/grafana/latest/panels-visualizations/)
