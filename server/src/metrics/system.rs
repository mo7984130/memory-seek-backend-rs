use metrics::gauge;
use std::path::PathBuf;
use sysinfo::{Disks, System};

/// 采集并更新系统资源指标.
pub fn collect_system_metrics(sys: &mut System) {
    sys.refresh_all();

    let pid = sysinfo::get_current_pid().expect("无法获取当前进程 PID");

    // CPU 指标
    let system_cpu = sys.global_cpu_usage();
    gauge!("system.cpu.usage").set(system_cpu as f64);
    gauge!("system.cpu.cores").set(sys.cpus().len() as f64);

    // 内存指标
    gauge!("system.memory.total").set(sys.total_memory() as f64);
    gauge!("system.memory.used").set(sys.used_memory() as f64);

    // 磁盘指标（取工作目录所在挂载点所在分区）
    collect_disk_metrics();

    // 进程级指标（一次查找，同时获取 CPU 与内存）
    match sys.process(pid) {
        Some(process) => {
            gauge!("system.cpu.process_usage").set(process.cpu_usage() as f64);
            gauge!("system.memory.process_usage").set(process.memory() as f64);
        }
        None => {
            common::caller_warn!("无法获取当前进程信息");
            gauge!("system.cpu.process_usage").set(-1.0);
            gauge!("system.memory.process_usage").set(-1.0);
        }
    }
}

/// 采集工作目录所在分区的磁盘总量与已用量
fn collect_disk_metrics() {
    let disks = Disks::new_with_refreshed_list();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let cwd_str = cwd.to_string_lossy();

    // 选择挂载点前缀匹配 cwd 且路径最长的磁盘（最精确的分区）
    let disk = disks
        .iter()
        .filter(|d| {
            let mount = d.mount_point().to_str().unwrap_or("/");
            cwd_str.starts_with(mount)
        })
        .max_by_key(|d| d.mount_point().to_str().unwrap_or("").len());

    match disk {
        Some(disk) => {
            let total = disk.total_space() as f64;
            let available = disk.available_space() as f64;
            gauge!("system.disk.total").set(total);
            gauge!("system.disk.used").set((total - available).max(0.0));
        }
        None => {
            gauge!("system.disk.total").set(-1.0);
            gauge!("system.disk.used").set(-1.0);
        }
    }
}
