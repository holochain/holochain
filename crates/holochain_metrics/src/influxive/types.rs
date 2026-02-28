#![deny(missing_docs)]
#![deny(warnings)]
#![deny(unsafe_code)]
//! Core types for influxive crates. The main point of this crate is to expose
//! the [MetricWriter] trait to be used by downstream influxive crates.

use std::borrow::Cow;
use std::sync::Arc;

/// Errors from influxive operations.
#[derive(thiserror::Error, Debug)]
pub enum InfluxiveError {
    /// IO errors (filesystem, process spawning, etc.)
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// HTTP request failures.
    #[cfg(feature = "download_binaries")]
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    /// HTTP download returned a non-success status.
    #[cfg(feature = "download_binaries")]
    #[error("Download failed: HTTP {0}")]
    DownloadFailed(u16),

    /// A command produced stderr output or failed to launch.
    #[error("Command error: {0}")]
    Command(String),

    /// Binary version doesn't match the expected version.
    #[error("Version mismatch: {0}")]
    VersionMismatch(String),

    /// Hash verification failed after download.
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch {
        /// The expected hash.
        expected: String,
        /// The actual hash.
        actual: String,
    },

    /// Influxd process failed to start or become ready.
    #[error("influxd startup failed: {0}")]
    Startup(String),

    /// Catch-all for miscellaneous errors.
    #[error("{0}")]
    Other(String),
}

/// Result type for influxive operations.
pub type InfluxiveResult<T> = Result<T, InfluxiveError>;

/// String type handling various string types usable by InfluxDB.
#[derive(Debug, Clone)]
pub enum StringType {
    /// String value.
    String(Cow<'static, str>),

    /// String value.
    ArcString(Arc<str>),
}

impl StringType {
    /// Get an owned string out of this StringType.
    pub fn into_string(self) -> String {
        match self {
            StringType::String(s) => s.into_owned(),
            StringType::ArcString(s) => s.to_string(),
        }
    }
}

macro_rules! stringtype_from_impl {
    ($($f:ty, $i:ident, $b:block,)*) => {$(
        impl From<$f> for StringType {
            fn from($i: $f) -> Self $b
        }
    )*};
}

stringtype_from_impl! {
    String, f, { StringType::String(Cow::Owned(f)) },
    &'static String, f, { StringType::String(Cow::Borrowed(f.as_str())) },
    &'static str, f, { StringType::String(Cow::Borrowed(f)) },
    Cow<'static, str>, f, { StringType::String(f) },
    Arc<str>, f, { StringType::ArcString(f) },
}

// TODO eliminate
/// Field-type enum for sending data to InfluxDB.
#[derive(Debug, Clone)]
pub enum DataType {
    /// Bool value.
    Bool(bool),

    /// Float value.
    F64(f64),

    /// Signed integer value.
    I64(i64),

    /// Unsigned integer value.
    U64(u64),

    /// String value.
    String(StringType),
}

macro_rules! datatype_from_impl {
    ($($f:ty, $i:ident, $b:block,)*) => {$(
        impl From<$f> for DataType {
            fn from($i: $f) -> Self $b
        }
    )*};
}

datatype_from_impl! {
    bool, f, { DataType::Bool(f) },
    f64, f, { DataType::F64(f) },
    f32, f, { DataType::F64(f as f64) },
    i8, f, { DataType::I64(f as i64) },
    i16, f, { DataType::I64(f as i64) },
    i32, f, { DataType::I64(f as i64) },
    i64, f, { DataType::I64(f as i64) },
    u8, f, { DataType::U64(f as u64) },
    u16, f, { DataType::U64(f as u64) },
    u32, f, { DataType::U64(f as u64) },
    u64, f, { DataType::U64(f as u64) },
    String, f, { DataType::String(f.into()) },
    &'static String, f, { DataType::String(f.into()) },
    &'static str, f, { DataType::String(f.into()) },
    Cow<'static, str>, f, { DataType::String(f.into()) },
    Arc<str>, f, { DataType::String(f.into()) },
}

/// A metric to record in the influxdb instance.
#[derive(Debug)]
pub struct Metric {
    /// The timestamp for this metric report.
    pub timestamp: std::time::SystemTime,

    /// The name of this metric report.
    pub name: String,

    /// The fields associated with this metric report.
    pub fields: Vec<(String, DataType)>,

    /// The tags associated with this metric report.
    pub tags: Vec<(String, DataType)>,
}

impl Metric {
    /// Construct a new metric report to be sent to InfluxDB.
    pub fn new(timestamp: std::time::SystemTime, name: impl Into<String>) -> Metric {
        Self {
            timestamp,
            name: name.into(),
            fields: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Add a field to this metric report.
    pub fn with_field<V>(mut self, name: impl Into<String>, value: V) -> Self
    where
        V: Into<DataType>,
    {
        self.fields.push((name.into(), value.into()));
        self
    }

    /// Add a tag to this metric report.
    pub fn with_tag<V>(mut self, name: impl Into<String>, value: V) -> Self
    where
        V: Into<DataType>,
    {
        self.tags.push((name.into(), value.into()));
        self
    }
}

/// Indicates a type that is capable of writing metrics to an InfluxDB instance.
pub trait MetricWriter: 'static + Send + Sync {
    /// Write a metric to an InfluxDB instance. Note, this should return
    /// immediately, perhaps by adding the metric to a memory buffer for
    /// a different process/task/thread to actually write the metric as
    /// determined by the concrete implementation.
    fn write_metric(&self, metric: Metric);
}

/// Trait object for MetricWriter
pub type DynMetricWriter = Arc<dyn MetricWriter>;
