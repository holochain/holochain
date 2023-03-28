//! # Metrics
//! WIP metrics helper for counting values
//! and sending tracing events.
//! This is designed to be fast so everything
//! is on the stack.
//! This means you need to keep the metric sets small (<100 metrics per set).
//! If you need more then make a new set.
use std::sync::atomic::AtomicBool;
#[allow(missing_docs)]
#[doc(hidden)]
static METRICS_ON: AtomicBool = AtomicBool::new(false);

/// Enable all metrics for your program
pub fn init() {
    METRICS_ON.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Is metrics currently enabled?
/// Call init() to enable.
pub fn is_enabled() -> bool {
    METRICS_ON.load(std::sync::atomic::Ordering::Relaxed)
}

/// Create a metrics set.
/// Takes the name of the metric set followed by
/// a list of metric names.
#[macro_export]
macro_rules! metrics {
    ($name:ident, $($metric:ident),+) => {
        #[allow(missing_docs)]
        #[derive(Debug, Copy, Clone)]
        pub enum $name {
            $($metric),+
        }

        mod metrics_inner {
            pub(crate) const NUM: usize = 0usize $(+ $crate::__replace_expr!($metric 1usize))+;
            pub(crate) static METRICS: [std::sync::atomic::AtomicU64; NUM] = [$($crate::__replace_expr!($metric std::sync::atomic::AtomicU64::new(0))),+];
            pub(crate) const NAMES: [&'static str; NUM] = [$(stringify!($metric)),+];
        }

        impl $name {
            /// Add to this counter and emit tracing event
            pub fn count<N, E>(metric: Self, n: N)
            where
                E: std::fmt::Debug,
                std::num::TryFromIntError: From<E>,
                N: std::convert::TryInto<u64, Error = E>,
            {
                $crate::metrics::__inner::count(&metrics_inner::METRICS[..], &metrics_inner::NAMES[..], metric as usize, n, "none")
            }
            /// Add to this counter and emit tracing event
            /// with a field that can be used as a filter.
            /// You can filter for this `[metric_count{filter=my_filter}]`.
            /// Or to get all without filters `[metric_count{filter=none}]`.
            pub fn count_filter<N, E>(metric: Self, n: N, filter: &str)
            where
                E: std::fmt::Debug,
                std::num::TryFromIntError: From<E>,
                N: std::convert::TryInto<u64, Error = E>,
            {
                $crate::metrics::__inner::count(&metrics_inner::METRICS[..], &metrics_inner::NAMES[..], metric as usize, n, filter)
            }
            /// Add to this counter without emit tracing event
            pub fn count_silent<N, E>(metric: Self, n: N) -> u64
            where
                E: std::fmt::Debug,
                std::num::TryFromIntError: From<E>,
                N: std::convert::TryInto<u64, Error = E>,
            {
                $crate::metrics::__inner::count_silent(&metrics_inner::METRICS[..], metric as usize, n)
            }
            /// Get the current value of this metric
            pub fn get(metric: Self) -> u64 {
                $crate::metrics::__inner::get(&metrics_inner::METRICS[..], metric as usize)
            }
            /// Get an iterator over all metrics
            pub fn iter() -> impl Iterator<Item = (Self, u64)> {
                $crate::metrics::__inner::iter(&metrics_inner::METRICS[..], &metrics_inner::NAMES[..])
                    .map(|(n, i)|(n.into(), i))
            }
            /// Emit tracing events for every metric
            pub fn print() {
                $crate::metrics::__inner::print(&metrics_inner::METRICS[..], &metrics_inner::NAMES[..])
            }
            /// Save all metrics to csv
            pub fn save_csv(path: &std::path::Path) {
                $crate::metrics::__inner::save_csv(&metrics_inner::METRICS[..], &metrics_inner::NAMES[..], path)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                use $name::*;
                match s {
                    $(stringify!($metric) => $metric),+,
                    _ => unreachable!("Tried to use a metric name that doesn't exist"),
                }
            }
        }

    };
}
#[macro_export]
#[allow(missing_docs)]
#[doc(hidden)]
macro_rules! __replace_expr {
    ($_t:tt $sub:expr) => {
        $sub
    };
}

#[allow(missing_docs)]
#[doc(hidden)]
pub mod __inner {
    use super::METRICS_ON;
    use std::sync::atomic::AtomicU64;
    pub fn count_silent<N, E>(metrics: &[AtomicU64], metric: usize, n: N) -> u64
    where
        E: std::fmt::Debug,
        std::num::TryFromIntError: From<E>,
        N: std::convert::TryInto<u64, Error = E>,
    {
        if METRICS_ON.load(std::sync::atomic::Ordering::Relaxed) {
            let n = n.try_into().expect("Failed to convert metric to u64");
            let mut last = metrics[metric].fetch_add(n, std::sync::atomic::Ordering::Relaxed);
            last += n;
            last
        } else {
            0
        }
    }
    pub fn count<N, E>(metrics: &[AtomicU64], names: &[&str], metric: usize, n: N, filter: &str)
    where
        E: std::fmt::Debug,
        std::num::TryFromIntError: From<E>,
        N: std::convert::TryInto<u64, Error = E>,
    {
        if METRICS_ON.load(std::sync::atomic::Ordering::Relaxed) {
            let n = n.try_into().expect("Failed to convert metric to u64");
            let r = count_silent::<_, std::convert::Infallible>(metrics, metric, n);
            let name = names[metric];
            let span = tracing::trace_span!("metric_count", %filter, %name);
            span.in_scope(|| {
                tracing::trace!(metric = %name, count = r, change = n);
            });
        }
    }
    pub fn get(metrics: &[AtomicU64], metric: usize) -> u64 {
        if METRICS_ON.load(std::sync::atomic::Ordering::Relaxed) {
            metrics[metric].load(std::sync::atomic::Ordering::Relaxed)
        } else {
            0
        }
    }
    pub fn iter(
        metrics: &'static [AtomicU64],
        names: &'static [&'static str],
    ) -> impl Iterator<Item = (&'static str, u64)> {
        metrics
            .iter()
            .zip(names.iter())
            .map(|(i, &name)| (name, i.load(std::sync::atomic::Ordering::Relaxed)))
    }
    pub fn print(metrics: &[AtomicU64], names: &[&str]) {
        if METRICS_ON.load(std::sync::atomic::Ordering::Relaxed) {
            let span = tracing::trace_span!("print_metrics");
            for (i, count) in metrics.iter().enumerate() {
                let metric = names[i];
                let count = count.load(std::sync::atomic::Ordering::Relaxed);
                span.in_scope(|| {
                    tracing::trace!(%metric, count);
                });
            }
        }
    }
    pub fn save_csv(metrics: &[AtomicU64], names: &[&str], path: &std::path::Path) {
        if METRICS_ON.load(std::sync::atomic::Ordering::Relaxed) {
            use std::fmt::Write;
            let mut keys = String::new();
            let mut values = String::new();
            for (count, metric) in metrics.iter().zip(names.iter()) {
                let count = count.load(std::sync::atomic::Ordering::Relaxed);
                write!(keys, "{},", metric).expect("Failed to write metrics");
                write!(values, "{},", count).expect("Failed to write metrics");
            }
            std::fs::write(path, format!("{}\n{}\n", keys, values))
                .expect("Failed to write metrics to csv");
            tracing::info!(metrics = "Saved csv to", ?path);
        }
    }
}
