//! Utilities for helping with metric tracking.

use crate::tracing;
use holochain_trace::tracing::Instrument;
use kitsune_p2p_bin_data::KitsuneAgent;
use kitsune_p2p_timestamp::Timestamp;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc, Once,
};
use sysinfo::ProcessesToUpdate;

static SYS_INFO: Once = Once::new();

static TASK_COUNT: AtomicUsize = AtomicUsize::new(0);
static USED_MEM_KB: AtomicU64 = AtomicU64::new(0);
static PROC_CPU_USAGE_PCT_1000: AtomicUsize = AtomicUsize::new(0);
static TX_BYTES_PER_SEC: AtomicU64 = AtomicU64::new(0);
static RX_BYTES_PER_SEC: AtomicU64 = AtomicU64::new(0);
static SENDS_PER_SEC: AtomicU64 = AtomicU64::new(0);
static RECVS_PER_SEC: AtomicU64 = AtomicU64::new(0);

macro_rules! _make_cntr {
    (
        $doc:literal,
        $push:ident,
        $pull:ident,
        $m:ident,
        $stat:ident,
    ) => {
        mod $m {
            use super::*;

            pub(crate) static $stat: AtomicU64 = AtomicU64::new(0);
        }

        #[doc = $doc]
        #[allow(dead_code)]
        pub(crate) fn $push(v: u64) {
            $m::$stat.fetch_add(v, Ordering::SeqCst);
        }

        pub(crate) fn $pull() -> u64 {
            $m::$stat
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| Some(0))
                .unwrap()
        }
    };
}

macro_rules! make_cntr {
    ($doc:literal, $push:ident, $pull:ident,) => {
        paste::paste! {
            _make_cntr!(
                $doc,
                $push,
                $pull,
                [<__ $push:snake>],
                [<$push:snake:upper>],
            );
        }
    };
}

make_cntr!(
    "count the raw number of messages sent out by this process",
    metric_push_raw_send_count,
    metric_pull_raw_send_count,
);

make_cntr!(
    "count the raw number of messages received by this process",
    metric_push_raw_recv_count,
    metric_pull_raw_recv_count,
);

macro_rules! _make_avg {
    (
        $doc:literal,
        $push:ident,
        $pull:ident,
        $m:ident,
        $stat:ident,
    ) => {
        mod $m {
            use super::*;

            pub(crate) static $stat: AtomicU64 = AtomicU64::new(0);
        }

        #[doc = $doc]
        #[allow(dead_code)]
        pub(crate) fn $push(v: u64) {
            // implement this as a bizzarre "drifting" average
            // old entries account for 4/5 of the weight while
            // new entries only effect the difference by one fifth,
            // so they won't start to dominate until > 10 events.
            $m::$stat
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, move |p| {
                    if p == 0 {
                        Some(v)
                    } else {
                        Some((p * 4 + v) / 5)
                    }
                })
                .unwrap();
        }

        pub(crate) fn $pull() -> u64 {
            $m::$stat.load(Ordering::SeqCst)
        }
    };
}

macro_rules! make_avg {
    ($doc:literal, $push:ident, $pull:ident,) => {
        paste::paste! {
            _make_avg!(
                $doc,
                $push,
                $pull,
                [<__ $push:snake>],
                [<$push:snake:upper>],
            );
        }
    };
}

make_avg!(
    "mark the size of a pool buf when released, so we can tune the shrink size",
    metric_push_pool_buf_release_size,
    metric_pull_pool_buf_release_size,
);

make_avg!(
    "kitsune api request / success response elapsed time",
    metric_push_api_req_res_elapsed_ms,
    metric_pull_api_req_res_elasped_ms,
);

/// Spawns a tokio task with given future/async block.
/// Captures a new TaskCounter instance to track task count.
pub fn metric_task<T, E, F>(f: F) -> tokio::task::JoinHandle<Result<T, E>>
where
    T: 'static + Send,
    E: 'static + Send + std::fmt::Debug,
    F: 'static + Send + std::future::Future<Output = Result<T, E>>,
{
    metric_task_instrumented(None, f)
}

/// Spawns a tokio task with given future/async block.
/// Captures a new TaskCounter instance to track task count.
pub fn metric_task_instrumented<T, E, F>(
    scope: Option<String>,
    f: F,
) -> tokio::task::JoinHandle<Result<T, E>>
where
    T: 'static + Send,
    E: 'static + Send + std::fmt::Debug,
    F: 'static + Send + std::future::Future<Output = Result<T, E>>,
{
    let counter = MetricTaskCounter::new();
    let task = async move {
        let _counter = counter;
        let res = f
            .instrument(tracing::info_span!("kitsune-metric-task", scope = scope))
            .await;
        if let Err(e) = &res {
            ghost_actor::dependencies::tracing::error!(?e, "METRIC TASK ERROR");
        }
        res
    };

    tokio::task::spawn(task)
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
    /// raw message send count (5 sec avg).
    pub sends_per_sec: u64,
    /// raw message recv count (5 sec avg).
    pub recvs_per_sec: u64,
    /// number of active tokio tasks
    pub tokio_task_count: usize,
    /// avg size of released pool bufs (bytes)
    pub avg_pool_buf_release_size: u64,
    /// avg api req/res elapsed time (ms)
    pub avg_api_req_res_elapsed_ms: u64,
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
                    .with_processes(sysinfo::ProcessRefreshKind::new().with_memory().with_cpu()),
            );
            let mut networks = sysinfo::Networks::new();

            let pid = sysinfo::get_current_pid().unwrap();
            let mut tx_avg = FiveAvg::new();
            let mut rx_avg = FiveAvg::new();
            let mut send_avg = FiveAvg::new();
            let mut recv_avg = FiveAvg::new();

            let mut p_count: usize = 0;

            loop {
                system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                networks.refresh_list();

                let proc = system.process(pid).unwrap();

                let mem = proc.memory();
                USED_MEM_KB.store(mem, Ordering::Relaxed);

                let cpu = (proc.cpu_usage() * 1000.0) as usize;
                PROC_CPU_USAGE_PCT_1000.store(cpu, Ordering::Relaxed);

                let mut tx = 0;
                let mut rx = 0;
                for (_n, network) in networks.iter() {
                    tx += network.transmitted();
                    rx += network.received();
                }
                tx_avg.push(tx);
                rx_avg.push(rx);
                TX_BYTES_PER_SEC.store(tx_avg.avg(), Ordering::Relaxed);
                RX_BYTES_PER_SEC.store(rx_avg.avg(), Ordering::Relaxed);

                send_avg.push(metric_pull_raw_send_count());
                SENDS_PER_SEC.store(send_avg.avg(), Ordering::Relaxed);
                recv_avg.push(metric_pull_raw_recv_count());
                RECVS_PER_SEC.store(recv_avg.avg(), Ordering::Relaxed);

                p_count += 1;
                if p_count == 15 {
                    p_count = 0;
                    let sys_info = get_sys_info();
                    tracing::info!(?sys_info);
                }

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
        sends_per_sec: SENDS_PER_SEC.load(Ordering::Relaxed),
        recvs_per_sec: RECVS_PER_SEC.load(Ordering::Relaxed),
        tokio_task_count: TASK_COUNT.load(Ordering::Relaxed),
        avg_pool_buf_release_size: metric_pull_pool_buf_release_size(),
        avg_api_req_res_elapsed_ms: metric_pull_api_req_res_elasped_ms(),
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

const METRIC_KIND_UNKNOWN: &str = "Unknown";
const METRIC_KIND_REACHABILITY_QUOTIENT: &str = "ReachabilityQuotient";
const METRIC_KIND_LATENCY_MICROS: &str = "LatencyMicros";
const METRIC_KIND_AGG_EXTRAP_COV: &str = "AggExtrapCov";

/// An individual metric record
#[derive(Debug)]
pub struct MetricRecord {
    /// kind of this record
    pub kind: MetricRecordKind,

    /// agent associated with this metric (if applicable)
    pub agent: Option<Arc<KitsuneAgent>>,

    /// timestamp this metric was recorded at
    pub recorded_at_utc: Timestamp,

    /// timestamp this metric will expire and be available for pruning
    pub expires_at_utc: Timestamp,

    /// additional data associated with this metric
    pub data: serde_json::Value,
}

/// The type of metric recorded
#[derive(Debug)]
pub enum MetricRecordKind {
    /// Failure to parse metric kind
    Unknown,

    /// ReachabilityQuotient metric kind
    ReachabilityQuotient,

    /// LatencyMicros metric kind
    LatencyMicros,

    /// AggExtrapCov metric kind
    AggExtrapCov,
}

impl MetricRecordKind {
    /// database format of this kind variant
    pub fn to_db(&self) -> &'static str {
        use MetricRecordKind::*;
        match self {
            Unknown => METRIC_KIND_UNKNOWN,
            ReachabilityQuotient => METRIC_KIND_REACHABILITY_QUOTIENT,
            LatencyMicros => METRIC_KIND_LATENCY_MICROS,
            AggExtrapCov => METRIC_KIND_AGG_EXTRAP_COV,
        }
    }

    /// parse a database kind into a rust enum variant
    pub fn from_db(input: &str) -> Self {
        use MetricRecordKind::*;
        if input == METRIC_KIND_REACHABILITY_QUOTIENT {
            ReachabilityQuotient
        } else if input == METRIC_KIND_LATENCY_MICROS {
            LatencyMicros
        } else if input == METRIC_KIND_AGG_EXTRAP_COV {
            AggExtrapCov
        } else {
            Unknown
        }
    }
}

type WriteLenCb = Box<dyn Fn(&'static str, usize) + 'static + Send + Sync>;

/// Metrics callback manager to be injected into the endpoint
pub struct Tx2ApiMetrics {
    write_len: Option<WriteLenCb>,
}

impl Default for Tx2ApiMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Tx2ApiMetrics {
    /// Construct a new default Tx2ApiMetrics with no set callbacks
    pub fn new() -> Self {
        Self { write_len: None }
    }

    /// This callback will be invoked when we successfully write data
    /// to a transport connection.
    pub fn set_write_len<F>(mut self, f: F) -> Self
    where
        F: Fn(&'static str, usize) + 'static + Send + Sync,
    {
        let f: WriteLenCb = Box::new(f);
        self.write_len = Some(f);
        self
    }

    #[allow(dead_code)]
    fn write_len(&self, d: &'static str, l: usize) {
        if let Some(cb) = &self.write_len {
            cb(d, l)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{get_sys_info, init_sys_info_poll, metric_task, TASK_COUNT};
    use std::sync::atomic::Ordering;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "flakey. https://app.circleci.com/jobs/github/holochain/holochain/12604"]
    async fn test_metric_task() {
        for _ in 0..20 {
            metric_task(async move {
                tokio::time::sleep(std::time::Duration::from_millis(3)).await;
                <Result<(), ()>>::Ok(())
            });
        }
        let gt_task_count = TASK_COUNT.load(Ordering::Relaxed);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let lt_task_count = TASK_COUNT.load(Ordering::Relaxed);
        assert!(lt_task_count < gt_task_count);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sys_info() {
        holochain_trace::test_run();
        init_sys_info_poll();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let sys_info = get_sys_info();
        ghost_actor::dependencies::tracing::info!(?sys_info);
    }
}
