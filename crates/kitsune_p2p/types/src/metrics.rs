//! Utilities for helping with metric tracking.

use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Once,
};
use sysinfo::{NetworkExt, NetworksExt, ProcessExt, SystemExt};

static SYS_INFO: Once = Once::new();

static TASK_COUNT: AtomicUsize = AtomicUsize::new(0);
static USED_MEM_KB: AtomicU64 = AtomicU64::new(0);
static PROC_CPU_USAGE_PCT_1000: AtomicUsize = AtomicUsize::new(0);
static TX_BYTES_PER_SEC: AtomicU64 = AtomicU64::new(0);
static RX_BYTES_PER_SEC: AtomicU64 = AtomicU64::new(0);

static POOL_BUF_RELEASE_SIZE_TOT: AtomicU64 = AtomicU64::new(0);
static POOL_BUF_RELEASE_SIZE_CNT: AtomicU64 = AtomicU64::new(0);

pub(crate) fn metric_push_pool_buf_release_size(size: usize) {
    POOL_BUF_RELEASE_SIZE_CNT.fetch_add(1, Ordering::SeqCst);
    let tot = POOL_BUF_RELEASE_SIZE_TOT.fetch_add(size as u64, Ordering::SeqCst);

    if tot > u64::MAX / 2 {
        POOL_BUF_RELEASE_SIZE_TOT.store(0, Ordering::SeqCst);
        POOL_BUF_RELEASE_SIZE_CNT.store(0, Ordering::SeqCst);
    }
}

fn metric_pull_pool_buf_release_size() -> u64 {
    let tot = POOL_BUF_RELEASE_SIZE_TOT.load(Ordering::SeqCst);
    let cnt = POOL_BUF_RELEASE_SIZE_CNT.load(Ordering::SeqCst);

    tot / cnt
}

/// Spawns a tokio task with given future/async block.
/// Captures a new TaskCounter instance to track task count.
pub fn metric_task<T, E, F>(f: F) -> tokio::task::JoinHandle<Result<T, E>>
where
    T: 'static + Send,
    E: 'static + Send + std::fmt::Debug,
    F: 'static + Send + std::future::Future<Output = Result<T, E>>,
{
    let counter = MetricTaskCounter::new();
    tokio::task::spawn(async move {
        let _counter = counter;
        let res = f.await;
        if let Err(e) = &res {
            ghost_actor::dependencies::tracing::error!(?e, "METRIC TASK ERROR");
        }
        res
    })
}

/// System Info.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricSysInfo {
    /// Used system memory KB.
    pub used_mem_kb: u64,
    /// Process CPU Usage % x1000.
    pub proc_cpu_usage_pct_1000: usize,
    /// network bytes transmitted (5 sec avg).
    pub tx_bytes_per_sec: u64,
    /// network bytes received (5 sec avg).
    pub rx_bytes_per_sec: u64,
    /// number of active tokio tasks
    pub tokio_task_count: usize,
    /// avg size of released pool bufs
    pub avg_pool_buf_release_size: u64,
}

/// Initialize polling of system usage info
pub fn init_sys_info_poll() {
    struct FiveAvg {
        idx: usize,
        data: [u64; 5],
    }

    impl FiveAvg {
        pub fn new() -> Self {
            Self {
                idx: 0,
                data: [0; 5],
            }
        }

        pub fn push(&mut self, val: u64) {
            self.data[self.idx] = val;
            self.idx += 1;
            if self.idx >= self.data.len() {
                self.idx = 0;
            }
        }

        pub fn avg(&self) -> u64 {
            let mut tot = 0;
            for f in self.data.iter() {
                tot += f;
            }
            tot / self.data.len() as u64
        }
    }

    SYS_INFO.call_once(|| {
        metric_task(async move {
            let mut system = sysinfo::System::new_with_specifics(
                sysinfo::RefreshKind::new()
                    .with_networks()
                    .with_networks_list(),
            );

            let pid = sysinfo::get_current_pid().unwrap();
            let mut tx_avg = FiveAvg::new();
            let mut rx_avg = FiveAvg::new();

            loop {
                system.refresh_process(pid);
                system.get_networks_mut().refresh();

                let proc = system.get_process(pid).unwrap();

                let mem = proc.memory();
                USED_MEM_KB.store(mem, Ordering::Relaxed);

                let cpu = (proc.cpu_usage() * 1000.0) as usize;
                PROC_CPU_USAGE_PCT_1000.store(cpu, Ordering::Relaxed);

                let mut tx = 0;
                let mut rx = 0;
                for (_n, network) in system.get_networks().iter() {
                    tx += network.get_transmitted();
                    rx += network.get_received();
                }
                tx_avg.push(tx);
                rx_avg.push(rx);
                TX_BYTES_PER_SEC.store(tx_avg.avg(), Ordering::Relaxed);
                RX_BYTES_PER_SEC.store(rx_avg.avg(), Ordering::Relaxed);

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }

            // this is needed for type-ing the block, but will never return
            #[allow(unreachable_code)]
            <Result<(), ()>>::Ok(())
        });
    });
}

/// Capture current sys_info metrics. Be sure you invoked `init_sys_info_poll`.
pub fn get_sys_info() -> MetricSysInfo {
    MetricSysInfo {
        used_mem_kb: USED_MEM_KB.load(Ordering::Relaxed),
        proc_cpu_usage_pct_1000: PROC_CPU_USAGE_PCT_1000.load(Ordering::Relaxed),
        tx_bytes_per_sec: TX_BYTES_PER_SEC.load(Ordering::Relaxed),
        rx_bytes_per_sec: RX_BYTES_PER_SEC.load(Ordering::Relaxed),
        tokio_task_count: TASK_COUNT.load(Ordering::Relaxed),
        avg_pool_buf_release_size: metric_pull_pool_buf_release_size(),
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

#[tokio::test(flavor = "multi_thread")]
async fn test_metric_task() {
    for _ in 0..20 {
        metric_task(async move {
            tokio::time::sleep(std::time::Duration::from_millis(3)).await;
            <Result<(), ()>>::Ok(())
        });
    }
    let gt_task_count = TASK_COUNT.load(Ordering::Relaxed);
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let lt_task_count = TASK_COUNT.load(Ordering::Relaxed);
    assert!(lt_task_count < gt_task_count);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sys_info() {
    observability::test_run().ok();
    init_sys_info_poll();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let sys_info = get_sys_info();
    ghost_actor::dependencies::tracing::info!(?sys_info);
}
