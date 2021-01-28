//! Utilities for helping with metric tracking.

use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Once,
};
use sysinfo::{ProcessExt, SystemExt};

static SYS_INFO: Once = Once::new();

static TASK_COUNT: AtomicUsize = AtomicUsize::new(0);
static USED_MEM: AtomicU64 = AtomicU64::new(0);
static PROC_CPU_USAGE: AtomicUsize = AtomicUsize::new(0);

/// Spawns a tokio task with given future/async block.
/// Captures a new TaskCounter instance to track task count.
#[macro_export]
macro_rules! metric_task {
    ($task:expr) => {{
        let counter = $crate::metrics::MetricTaskCounter::new();
        let fut = { $task };
        ::tokio::task::spawn(async move {
            let _counter = counter;
            fut.await
        })
    }};
}

/// System Info.
#[derive(Debug, Clone)]
pub struct MetricSysInfo {
    /// Used system memory.
    pub used_mem: u64,
    /// Process CPU Usage % x1000.
    pub proc_cpu_usage: usize,
}

/// Initialize polling of system usage info
pub fn init_sys_info_poll() {
    SYS_INFO.call_once(|| {
        metric_task!(async move {
            let mut system = sysinfo::System::new();
            let pid = sysinfo::get_current_pid().unwrap();

            loop {
                system.refresh_process(pid);

                let proc = system.get_process(pid).unwrap();

                let mem = proc.memory();
                USED_MEM.store(mem, Ordering::Relaxed);

                let cpu = (proc.cpu_usage() * 1000.0) as usize;
                PROC_CPU_USAGE.store(cpu, Ordering::Relaxed);

                tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
            }
        });
    });
}

/// Capture current sys_info metrics. Be sure you invoked `init_sys_info_poll`.
pub fn get_sys_info() -> MetricSysInfo {
    MetricSysInfo {
        used_mem: USED_MEM.load(Ordering::Relaxed),
        proc_cpu_usage: PROC_CPU_USAGE.load(Ordering::Relaxed),
    }
}

/// Increases task count on `TaskCounter::new()`, decreases on drop.
pub struct MetricTaskCounter(());

impl Drop for MetricTaskCounter {
    fn drop(&mut self) {
        TASK_COUNT.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Default for MetricTaskCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricTaskCounter {
    /// Increase task count while intance exists, decrease when dropped.
    pub fn new() -> Self {
        TASK_COUNT.fetch_add(1, Ordering::Relaxed);
        Self(())
    }
}

/// Fetch the count of running tokio::tasks started with `metric_task!()`.
pub fn metric_task_count() -> usize {
    TASK_COUNT.load(Ordering::Relaxed)
}

#[tokio::test(threaded_scheduler)]
async fn test_metric_task() {
    for _ in 0..20 {
        metric_task!(async move {
            tokio::time::delay_for(std::time::Duration::from_millis(3)).await;
        });
    }
    let gt_task_count = metric_task_count();
    tokio::time::delay_for(std::time::Duration::from_millis(5)).await;
    let lt_task_count = metric_task_count();
    assert!(lt_task_count < gt_task_count);
}

#[tokio::test(threaded_scheduler)]
async fn test_sys_info() {
    init_sys_info_poll();
    tokio::time::delay_for(std::time::Duration::from_millis(5)).await;
    let _ = get_sys_info();
}
