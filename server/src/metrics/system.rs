use metrics::gauge;
use sysinfo::System;
use tracing::warn;

pub fn collect_system_metrics(sys: &mut System) {
    sys.refresh_all();

    let pid = sysinfo::get_current_pid().expect("无法获取当前进程 PID");

    // CPU 指标
    let system_cpu = sys.global_cpu_usage();
    gauge!("system.cpu.usage").set(system_cpu as f64);

    // 内存指标
    gauge!("system.memory.total").set(sys.total_memory() as f64);
    gauge!("system.memory.used").set(sys.used_memory() as f64);

    // 进程级指标（一次查找，同时获取 CPU 和内存）
    match sys.process(pid) {
        Some(process) => {
            gauge!("system.cpu.process_usage").set(process.cpu_usage() as f64);
            gauge!("system.memory.process_usage").set(process.memory() as f64);
        }
        None => {
            warn!("无法获取当前进程信息");
            gauge!("system.cpu.process_usage").set(-1.0);
            gauge!("system.memory.process_usage").set(-1.0);
        }
    }
}
