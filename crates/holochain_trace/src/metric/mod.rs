//! Wrappers for working with OpenTelemetry metrics

mod init;
mod task_run;

pub use init::init_metrics;
pub use task_run::TaskRunMetric;
