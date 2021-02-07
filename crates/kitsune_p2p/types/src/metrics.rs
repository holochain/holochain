//! Utilities for helping with metric tracking.

use futures::FutureExt;
use spawn_pressure::spawn_queue_limit;
use spawn_pressure::SpawnLimit;
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

/// Spawns a tokio task with given future/async block.
/// Captures a new TaskCounter instance to track task count.
pub async fn metric_task<T, E, F>(
    limit: &'static SpawnLimit,
    f: F,
) -> tokio::task::JoinHandle<Result<T, E>>
where
    T: 'static + Send,
    E: 'static + Send + std::fmt::Debug,
    F: 'static + Send + std::future::Future<Output = Result<T, E>>,
{
    metric_task_warn_limit(limit, f)
}

/// Blocks before spawning past limit
pub async fn metric_task_block<T, E, F>(
    limit: &'static SpawnLimit,
    f: F,
) -> tokio::task::JoinHandle<Result<T, E>>
where
    T: 'static + Send,
    E: 'static + Send + std::fmt::Debug,
    F: 'static + Send + std::future::Future<Output = Result<T, E>>,
{
    let mut full = false;
    let ref mut full_ref = full;
    let start = std::time::Instant::now();
    let r = spawn_queue_limit(
        limit,
        || {
            *full_ref = true;
            match limit.show_location() {
                Some((file, line)) => {
                    observability::tracing::error!(
                        "Spawning task at {}:{} hit limit {}",
                        file,
                        line,
                        limit.show_limit()
                    );
                }
                None => {
                    observability::tracing::error!(
                        "Spawning task hit limit {}",
                        limit.show_limit()
                    );
                }
            }
        },
        metric_inner(f).boxed(),
    )
    .await;
    if full {
        let t = start.elapsed();
        match limit.show_location() {
            Some((file, line)) => {
                let msg = format!(
                    "Spawning task at {}:{} hit limit {}",
                    file,
                    line,
                    limit.show_limit()
                );
                observability::tracing::error!(
                    ?msg,
                    waited = ?t,
                );
            }
            None => {
                let msg = format!("Spawning task hit limit {}", limit.show_limit());
                observability::tracing::error!(
                    ?msg,
                    waited = ?t,
                );
            }
        }
    }
    r
}

/// Same as metric task but will never
/// block, returns an error with your task if limit is reached.
pub fn metric_task_try_limit<T, E, F>(
    limit: &'static SpawnLimit,
    f: F,
) -> Result<tokio::task::JoinHandle<Result<T, E>>, F>
where
    T: 'static + Send,
    E: 'static + Send + std::fmt::Debug,
    F: 'static + Send + std::future::Future<Output = Result<T, E>>,
{
    match limit.take_limit() {
        Some(guard) => {
            let jh = guard.spawn(metric_inner(f));
            Ok(jh)
        }
        None => Err(f),
    }
}

/// Same as metric task but will never
/// block, instead an error will be logged.
pub fn metric_task_warn_limit<T, E, F>(
    limit: &'static SpawnLimit,
    f: F,
) -> tokio::task::JoinHandle<Result<T, E>>
where
    T: 'static + Send,
    E: 'static + Send + std::fmt::Debug,
    F: 'static + Send + std::future::Future<Output = Result<T, E>>,
{
    match metric_task_try_limit(limit, f) {
        Ok(jh) => jh,
        Err(f) => {
            match limit.show_location() {
                Some((file, line)) => {
                    observability::tracing::error!(
                        "Spawning task at {}:{} beyond limit {}",
                        file,
                        line,
                        limit.show_limit()
                    );
                }
                None => {
                    observability::tracing::error!(
                        "Spawning task beyond limit {}",
                        limit.show_limit()
                    );
                }
            }
            tokio::task::spawn(metric_inner(f))
        }
    }
}

fn metric_inner<T, E, F>(f: F) -> impl std::future::Future<Output = Result<T, E>>
where
    T: 'static + Send,
    E: 'static + Send + std::fmt::Debug,
    F: 'static + Send + std::future::Future<Output = Result<T, E>>,
{
    let counter = MetricTaskCounter::new();
    async move {
        let _counter = counter;
        let res = f.await;
        if let Err(e) = &res {
            ghost_actor::dependencies::tracing::error!(?e, "METRIC TASK ERROR");
        }
        res
    }
}

/// System Info.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricSysInfo {
    /// Used system memory KB.
    pub used_mem_kb: u64,
    /// Process CPU Usage % x1000.
    pub proc_cpu_usage_pct_1000: usize,
    /// network bytes transmitted.
    pub tx_bytes_per_sec: u64,
    /// network bytes received.
    pub rx_bytes_per_sec: u64,
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
        metric_task_warn_limit(spawn_pressure::spawn_limit!(1000), async move {
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

                tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
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
        metric_task_warn_limit(spawn_pressure::spawn_limit!(20), async move {
            tokio::time::delay_for(std::time::Duration::from_millis(3)).await;
            <Result<(), ()>>::Ok(())
        });
    }
    let gt_task_count = metric_task_count();
    tokio::time::delay_for(std::time::Duration::from_millis(5)).await;
    let lt_task_count = metric_task_count();
    assert!(lt_task_count < gt_task_count);
}

#[tokio::test(threaded_scheduler)]
async fn test_sys_info() {
    observability::test_run().ok();
    init_sys_info_poll();
    tokio::time::delay_for(std::time::Duration::from_millis(200)).await;
    let sys_info = get_sys_info();
    ghost_actor::dependencies::tracing::info!(?sys_info);
}
