#![deny(missing_docs)]
#![deny(warnings)]
#![deny(unsafe_code)]
//! Rust utility for efficiently writing metrics to InfluxDB.
//! Metrics can be written directly to a running InfluxDB instance or
//! written to a Line Protocol file on disk that can be pushed to InfluxDB using Telegraf.

use super::types::*;
use std::sync::Arc;

trait DataTypeExt {
    fn into_type(self) -> influxdb::Type;
}

impl DataTypeExt for DataType {
    fn into_type(self) -> influxdb::Type {
        match self {
            DataType::Bool(b) => influxdb::Type::Boolean(b),
            DataType::F64(f) => influxdb::Type::Float(f),
            DataType::I64(i) => influxdb::Type::SignedInteger(i),
            DataType::U64(u) => influxdb::Type::UnsignedInteger(u),
            DataType::String(s) => influxdb::Type::Text(s),
        }
    }
}

/// Backend types you probably don't need.
pub mod types {
    use super::*;
    use influxdb::Query;
    use tokio::io::AsyncWriteExt;

    /// backend
    pub trait Backend: 'static + Send + Sync {
        /// buffer a metric
        fn buffer_metric(&mut self, metric: Metric);

        /// get count of buffered metrics
        fn buffer_count(&self) -> usize;

        /// send buffered metrics
        fn send(
            &mut self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_ + Send + Sync>>;
    }

    /// factory
    pub trait BackendFactory: std::fmt::Debug + 'static + Send + Sync {
        /// create a new influxdb backend connector via token auth
        fn with_token_auth(
            &self,
            host: String,
            bucket: String,
            token: String,
        ) -> Box<dyn Backend + 'static + Send + Sync>;
    }

    struct DefaultBackend {
        buffer: Vec<influxdb::WriteQuery>,
        client: influxdb::Client,
    }

    impl Backend for DefaultBackend {
        fn buffer_metric(&mut self, metric: Metric) {
            self.buffer.push(metric_to_query(metric));
        }

        fn buffer_count(&self) -> usize {
            self.buffer.len()
        }

        fn send(
            &mut self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_ + Send + Sync>> {
            Box::pin(async move {
                if let Err(err) = self.client.query(std::mem::take(&mut self.buffer)).await {
                    tracing::warn!(?err, "write metrics error");
                }
            })
        }
    }

    /// currently backed by the crate influxdb,
    /// but subject to change without notice
    #[derive(Debug)]
    pub struct DefaultBackendFactory;

    impl BackendFactory for DefaultBackendFactory {
        fn with_token_auth(
            &self,
            host: String,
            bucket: String,
            token: String,
        ) -> Box<dyn Backend + 'static + Send + Sync> {
            let client = influxdb::Client::new(host, bucket).with_token(token);
            let out: Box<dyn Backend + 'static + Send + Sync> = Box::new(DefaultBackend {
                buffer: Vec::new(),
                client,
            });
            out
        }
    }

    struct LineProtocolFileBackend {
        buffer: Vec<influxdb::WriteQuery>,
        writer: tokio::io::BufWriter<tokio::fs::File>,
    }

    impl Backend for LineProtocolFileBackend {
        fn buffer_metric(&mut self, metric: Metric) {
            self.buffer.push(metric_to_query(metric));
        }

        fn buffer_count(&self) -> usize {
            self.buffer.len()
        }

        fn send(
            &mut self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_ + Send + Sync>> {
            Box::pin(async move {
                let buffer = std::mem::take(&mut self.buffer);
                for query in buffer {
                    match query.build_with_opts(true) {
                        Err(err) => tracing::warn!(?err, "write metrics error"),
                        Ok(v) => {
                            let line = format!("{}\n", v.get());
                            if let Err(err) = self.writer.write_all(line.as_bytes()).await {
                                tracing::warn!(?err, "write metrics error");
                            }
                        }
                    }
                }
                if let Err(err) = self.writer.flush().await {
                    tracing::warn!(?err, "write metrics error");
                }
            })
        }
    }

    /// Use InfluxDB Line Protocol
    #[derive(Debug)]
    pub struct LineProtocolFileBackendFactory {
        file_path: std::path::PathBuf,
    }

    impl LineProtocolFileBackendFactory {
        /// Creates a new instance with the provided file path.
        pub fn new(file_path: std::path::PathBuf) -> Self {
            Self { file_path }
        }
    }

    impl BackendFactory for LineProtocolFileBackendFactory {
        fn with_token_auth(
            &self,
            _host: String,
            _bucket: String,
            _token: String,
        ) -> Box<dyn Backend + 'static + Send + Sync> {
            let file = match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
            {
                Ok(file) => tokio::fs::File::from_std(file),
                Err(e) => panic!("Failed to create file at path: {:?}: {}", self.file_path, e),
            };
            let writer = tokio::io::BufWriter::new(file);
            let out: Box<dyn Backend + 'static + Send + Sync> = Box::new(LineProtocolFileBackend {
                buffer: Vec::new(),
                writer,
            });
            out
        }
    }
}

/// InfluxDB metric writer configuration.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct InfluxiveWriterConfig {
    /// Max time span over which metric writes will be buffered before
    /// actually being written to InfluxDB to facilitate batching.
    /// Defaults to `100ms`.
    pub batch_duration: std::time::Duration,

    /// The size of the metric write batch buffer. If a metric to be
    /// written goes beyond this buffer, the batch will be sent early.
    /// If the buffer is again full before the previous batch finishes
    /// sending, the metric will be ignored and a trace will be written
    /// at "debug" level.
    /// Defaults to `4096`.
    pub batch_buffer_size: usize,

    /// Backend driving this writer instance. This is currently driven
    /// by the influxdb crate, but that is subject to change without notice.
    pub backend: Arc<dyn types::BackendFactory + 'static + Send + Sync>,
}

impl Default for InfluxiveWriterConfig {
    fn default() -> Self {
        Self {
            batch_duration: std::time::Duration::from_millis(100),
            batch_buffer_size: 4096,
            backend: Arc::new(types::DefaultBackendFactory),
        }
    }
}

impl InfluxiveWriterConfig {
    /// Construct a Config that uses a LineProtocolFileBackendFactory
    pub fn create_with_influx_file(path: std::path::PathBuf) -> Self {
        Self {
            batch_duration: std::time::Duration::from_millis(100),
            batch_buffer_size: 4096,
            backend: Arc::new(types::LineProtocolFileBackendFactory::new(path)),
        }
    }

    /// Apply [InfluxiveWriterConfig::batch_duration].
    pub fn with_batch_duration(mut self, batch_duration: std::time::Duration) -> Self {
        self.batch_duration = batch_duration;
        self
    }

    /// Apply [InfluxiveWriterConfig::batch_buffer_size].
    pub fn with_batch_buffer_size(mut self, batch_buffer_size: usize) -> Self {
        self.batch_buffer_size = batch_buffer_size;
        self
    }

    /// Apply [InfluxiveWriterConfig::backend].
    pub fn with_backend(
        mut self,
        backend: Arc<dyn types::BackendFactory + 'static + Send + Sync>,
    ) -> Self {
        self.backend = backend;
        self
    }
}

enum WriteCmd {
    Timeout,
    Metric(Metric),
}

struct WriteBuf {
    config: InfluxiveWriterConfig,
    backend: Box<dyn types::Backend + 'static + Send + Sync>,
    last_send: std::time::Instant,
}

type ShouldSend = bool;

impl WriteBuf {
    pub fn new(
        config: InfluxiveWriterConfig,
        backend: Box<dyn types::Backend + 'static + Send + Sync>,
    ) -> Self {
        Self {
            config,
            backend,
            last_send: std::time::Instant::now(),
        }
    }

    pub fn process(&mut self, cmd: WriteCmd) -> ShouldSend {
        match cmd {
            WriteCmd::Timeout => {
                self.backend.buffer_count() > 0
                    && self.last_send.elapsed() >= self.config.batch_duration
            }
            WriteCmd::Metric(metric) => {
                if self.backend.buffer_count() == 0 {
                    self.last_send = std::time::Instant::now();
                }

                self.backend.buffer_metric(metric);

                self.backend.buffer_count() >= self.config.batch_buffer_size
                    || self.last_send.elapsed() >= self.config.batch_duration
            }
        }
    }

    pub async fn send(&mut self) {
        self.backend.send().await;
    }
}

/// InfluxDB metric writer instance.
pub struct InfluxiveWriter(tokio::sync::mpsc::Sender<WriteCmd>);

impl InfluxiveWriter {
    /// Construct a new writer authenticated by a token.
    pub fn with_token_auth<H: AsRef<str>, B: AsRef<str>, T: AsRef<str>>(
        config: InfluxiveWriterConfig,
        host: H,
        bucket: B,
        token: T,
    ) -> Self {
        let backend = config.backend.with_token_auth(
            host.as_ref().to_string(),
            bucket.as_ref().to_string(),
            token.as_ref().to_string(),
        );

        let (write_send, mut write_recv) = tokio::sync::mpsc::channel(config.batch_buffer_size);

        let write_send_timer = write_send.clone();
        let mut interval = tokio::time::interval(config.batch_duration / 3);
        tokio::task::spawn(async move {
            loop {
                interval.tick().await;
                if write_send_timer.send(WriteCmd::Timeout).await.is_err() {
                    break;
                }
            }
        });

        let mut write_buf = WriteBuf::new(config, backend);

        tokio::task::spawn(async move {
            while let Some(cmd) = write_recv.recv().await {
                if write_buf.process(cmd) {
                    write_buf.send().await;
                }

                loop {
                    match write_recv.try_recv() {
                        Ok(cmd) => {
                            if write_buf.process(cmd) {
                                write_buf.send().await;
                            }
                        }
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => return,
                    }
                }
            }
        });

        Self(write_send)
    }

    /// Log a metric to the running InfluxDB instance.
    /// Note, this function itself is an efficiency abstraction,
    /// which will return quickly if there is space in the buffer.
    /// The actual call to log the metrics will be made a configurable
    /// timespan later to facilitate batching of metric writes.
    pub fn write_metric(&self, metric: Metric) {
        match self.0.try_send(WriteCmd::Metric(metric)) {
            Ok(()) => (),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!("metrics overloaded, dropping metric");
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                /* ignore this, can happen during shutdown */
            }
        }
    }
}

impl super::types::MetricWriter for InfluxiveWriter {
    fn write_metric(&self, metric: Metric) {
        InfluxiveWriter::write_metric(self, metric);
    }
}

/// Converts a Metric to a WriteQuery
fn metric_to_query(metric: Metric) -> influxdb::WriteQuery {
    let Metric {
        timestamp,
        name,
        fields,
        tags,
    } = metric;

    let mut query = influxdb::WriteQuery::new(
        influxdb::Timestamp::Nanoseconds(
            timestamp
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("invalid system time")
                .as_nanos(),
        ),
        name,
    );

    for (k, v) in fields {
        query = query.add_field(k, v.into_type());
    }

    for (k, v) in tags {
        query = query.add_tag(k, v.into_type());
    }

    query
}

#[cfg(test)]
mod test;
