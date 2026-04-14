# 分离 basic_metrics Feature 计划

## 背景

当前 `metrics` feature 包含两部分功能：
1. **基础监控**（metrics.rs 中的两个 task）：
   - `track_database_metrics`: 监控数据库连接池指标（活跃连接、空闲连接、最大连接数）
   - `track_system_metrics`: 监控系统指标（使用 metrics-process collector）

2. **业务指标监控**：
   - 在 domains/user 和 domains/auth 中使用 metrics 记录业务指标

用户希望将基础监控分离为独立的 `basic_metrics` feature。

## 实施步骤

### 1. 修改 server/Cargo.toml

#### 1.1 添加 basic_metrics feature
```toml
[features]
default = ["auth", "user", "photo", "metrics"]
auth = ["dep:auth"]
user = ["dep:user", "auth"]
photo = ["dep:photo", "user", "dep:face_engine"]
basic_metrics = [
    "dep:metrics",
    "dep:metrics-exporter-prometheus",
    "dep:metrics-tracing-context",
    "dep:metrics-util",
    "dep:metrics-process"
]
metrics = [
    "basic_metrics",  # metrics 依赖 basic_metrics
    "common/metrics",
]
```

#### 1.2 说明
- `basic_metrics` 包含基础监控所需的所有依赖
- `metrics` feature 依赖 `basic_metrics` 并额外启用 `common/metrics`
- 这样可以保持向后兼容，现有的 `metrics` feature 仍然包含所有功能

### 2. 修改 server/src/metrics.rs

#### 2.1 更新条件编译属性
将所有 `#[cfg(feature = "metrics")]` 改为 `#[cfg(feature = "basic_metrics")]`：

```rust
#[cfg(feature = "basic_metrics")]
use metrics_exporter_prometheus::PrometheusBuilder;
#[cfg(feature = "basic_metrics")]
use metrics_process::Collector;
// ... 其他导入

#[cfg(feature = "basic_metrics")]
pub fn init_metrics_system() {
    // ...
}

#[cfg(not(feature = "basic_metrics"))]
pub fn init_metrics_system() {
}

#[cfg(feature = "basic_metrics")]
pub fn spawn_monitoring_tasks(db: DatabaseConnection) {
    tokio::spawn(track_database_metrics(db));
    tokio::spawn(track_system_metrics());
}

#[cfg(not(feature = "basic_metrics"))]
pub fn spawn_monitoring_tasks(_db: DatabaseConnection) {
}

#[cfg(feature = "basic_metrics")]
async fn track_database_metrics(db: DatabaseConnection) {
    // ...
}

#[cfg(feature = "basic_metrics")]
async fn track_system_metrics() {
    // ...
}
```

### 3. 修改 server/src/main.rs

#### 3.1 更新模块导入
```rust
#[cfg(feature = "basic_metrics")]
mod metrics;

#[cfg(feature = "basic_metrics")]
use crate::metrics::{init_metrics_system, spawn_monitoring_tasks};

#[cfg(feature = "basic_metrics")]
use metrics_tracing_context::MetricsLayer;
```

#### 3.2 更新使用位置
```rust
#[cfg(feature = "basic_metrics")]
let registry = registry.with(MetricsLayer::new());

#[cfg(feature = "basic_metrics")]
init_metrics_system();

#[cfg(feature = "basic_metrics")]
spawn_monitoring_tasks(db.clone());
```

### 4. 验证 domains 中的 metrics 使用

domains/user 和 domains/auth 中的 metrics 使用保持不变，因为：
- 它们使用 `#[cfg(feature = "metrics")]` 条件编译
- 当启用 `metrics` feature 时，`basic_metrics` 会自动启用
- 它们依赖 `common/metrics`，这在 `metrics` feature 中仍然存在

### 5. 测试场景

#### 5.1 只启用 basic_metrics
```bash
cargo build --no-default-features --features basic_metrics
```
预期结果：
- 基础监控（数据库、系统指标）正常工作
- 业务指标监控不工作

#### 5.2 启用 metrics（默认）
```bash
cargo build
```
预期结果：
- 基础监控正常工作
- 业务指标监控正常工作

#### 5.3 不启用任何 metrics
```bash
cargo build --no-default-features --features auth,user,photo
```
预期结果：
- 所有监控功能都不工作

## 依赖关系图

```
basic_metrics (基础监控)
    ├── metrics crate
    ├── metrics-exporter-prometheus
    ├── metrics-tracing-context
    ├── metrics-util
    └── metrics-process

metrics (完整监控)
    ├── basic_metrics
    └── common/metrics
        └── metrics crate

domains/user
    └── metrics feature
        ├── common/metrics
        └── metrics crate

domains/auth
    └── metrics feature
        ├── common/metrics
        └── metrics crate
```

## 优势

1. **灵活性**：用户可以选择只启用基础监控，而不需要业务指标
2. **向后兼容**：现有的 `metrics` feature 仍然包含所有功能
3. **清晰的职责分离**：基础监控和业务指标监控分开
4. **减少依赖**：如果只需要基础监控，可以避免引入 common/metrics 的其他依赖
