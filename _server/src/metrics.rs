use log::info;
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_process::Collector;
use metrics_tracing_context::TracingContextLayer;
use metrics_util::layers::Stack;
use sea_orm::DatabaseConnection;
use std::time::Duration;
use tokio::time;

/// 初始化 Prometheus 指标系统
///
/// 创建 Prometheus exporter 并在 `0.0.0.0:9000` 上启动 HTTP 监听，
/// 设置全局指标记录器，仅允许 `status`、`reason`、`service` 三个 tracing 标签。
pub fn init_metrics_system() {
    let (recorder, exporter) = PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .build()
        .expect("failed to build prometheus");

    let decorated_recorder = Stack::new(recorder).push(TracingContextLayer::only_allow([
        "status", "reason", "service",
    ]));

    metrics::set_global_recorder(decorated_recorder)
        .expect("failed to set metrics global recorder");

    tokio::spawn(exporter);

    info!("metrics exporter 启动成功")
}

/// 启动后台监控任务
///
/// 分别启动数据库连接池指标采集和系统指标采集两个异步任务。
///
/// # 参数
/// - `db`: 数据库连接，用于采集连接池指标
pub fn spawn_monitoring_tasks(db: DatabaseConnection) {
    tokio::spawn(track_database_metrics(db));
    tokio::spawn(track_system_metrics());
}

// 周期性采集数据库连接池指标（活跃连接数、空闲连接数、最大连接数），每 5 秒一次
async fn track_database_metrics(db: DatabaseConnection) {
    let mut interval = time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let pool = db.get_postgres_connection_pool();
        let total_size = pool.size() as i32;
        let idle_count = pool.num_idle() as i32;
        let active_count = total_size - idle_count;

        metrics::gauge!("db_pool_active").set(active_count);
        metrics::gauge!("db_pool_idle").set(idle_count);

        let max_conn = pool.options().get_max_connections();
        metrics::gauge!("db_pool_max_connections").set(max_conn);
    }
}

// 周期性采集系统进程指标（CPU、内存等），每 5 秒一次
async fn track_system_metrics() {
    let mut interval = time::interval(Duration::from_secs(5));
    let collector = Collector::default();
    collector.describe();

    loop {
        interval.tick().await;
        collector.collect()
    }
}
