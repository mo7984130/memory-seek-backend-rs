use metrics::gauge;
use sysinfo::System;
use tracing::warn;

/// 采集系统级指标（CPU、内存）
///
/// 失败时设置 -1 并记录 warn 日志
pub fn collect_system_metrics(sys: &mut System) {
    sys.refresh_all();

    // CPU 指标
    let system_cpu = sys.global_cpu_usage();
    gauge!("system.cpu.usage").set(system_cpu as f64);

    let process_cpu = sys
        .process(sysinfo::get_current_pid().unwrap())
        .map(|p| p.cpu_usage() as f64)
        .unwrap_or_else(|| {
            warn!("无法获取进程 CPU 使用率");
            -1.0
        });
    gauge!("system.cpu.process_usage").set(process_cpu);

    // 内存指标
    gauge!("system.memory.total").set(sys.total_memory() as f64);
    gauge!("system.memory.used").set(sys.used_memory() as f64);

    let process_memory = sys
        .process(sysinfo::get_current_pid().unwrap())
        .map(|p| p.memory() as f64)
        .unwrap_or_else(|| {
            warn!("无法获取进程内存使用量");
            -1.0
        });
    gauge!("system.memory.process_usage").set(process_memory);
}
