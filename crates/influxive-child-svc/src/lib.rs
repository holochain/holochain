#![deny(missing_docs)]
#![deny(unsafe_code)]
//! Run influxd as a child process.
//!
//! ## Example
//!
//! ```
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() {
//! use influxive_core::Metric;
//! use influxive_child_svc::*;
//!
//! let tmp = tempfile::tempdir().unwrap();
//!
//! let influxive = InfluxiveChildSvc::new(
//!     InfluxiveChildSvcConfig::default()
//!         .with_database_path(Some(tmp.path().to_owned())),
//! ).await.unwrap();
//!
//! influxive.write_metric(
//!     Metric::new(
//!         std::time::SystemTime::now(),
//!         "my.metric",
//!     )
//!     .with_field("value", 3.14)
//!     .with_tag("tag", "test-tag")
//! );
//! # }
//! ```

use std::io::Result;

#[cfg(feature = "download_binaries")]
mod download_binaries;

use influxive_core::*;
use influxive_writer::*;

pub use influxive_writer::InfluxiveWriterConfig;

macro_rules! cmd_output {
    ($cmd:expr $(,$arg:expr)*) => {async {
        let mut proc = tokio::process::Command::new($cmd);
        proc.stdin(std::process::Stdio::null());
        proc.kill_on_drop(true);
        $(
            proc.arg($arg);
        )*
        let output = proc.output().await?;
        let err = String::from_utf8_lossy(&output.stderr);
        if !err.is_empty() {
            Err(err_other(err.to_string()))
        } else {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
    }.await}
}

/// Configure the child process.
#[derive(Debug)]
#[non_exhaustive]
pub struct InfluxiveChildSvcConfig {
    /// If true, will fall back to downloading influx release binaries.
    /// Defaults to `true`.
    #[cfg(feature = "download_binaries")]
    pub download_binaries: bool,

    /// Path to influxd binary. If None, will try the path.
    /// Defaults to `None`.
    pub influxd_path: Option<std::path::PathBuf>,

    /// Path to influx cli binary. If None, will try the path.
    /// Defaults to `None`.
    pub influx_path: Option<std::path::PathBuf>,

    /// Path to influx database files and config directory. If None, will
    /// use `./influxive`.
    /// Defaults to `None`.
    pub database_path: Option<std::path::PathBuf>,

    /// Influx initial username.
    /// Defaults to `influxive`.
    pub user: String,

    /// Influx initial password.
    /// Defaults to `influxive`.
    pub pass: String,

    /// Influx initial organization name.
    /// Defaults to `influxive`.
    pub org: String,

    /// Influx initial bucket name.
    /// Defaults to `influxive`.
    pub bucket: String,

    /// Retention timespan.
    /// Defaults to `72h`.
    pub retention: String,

    /// The influxive metric writer configuration.
    pub metric_write: InfluxiveWriterConfig,
}

impl Default for InfluxiveChildSvcConfig {
    fn default() -> Self {
        Self {
            #[cfg(feature = "download_binaries")]
            download_binaries: true,
            influxd_path: None,
            influx_path: None,
            database_path: None,
            user: "influxive".to_string(),
            pass: "influxive".to_string(),
            org: "influxive".to_string(),
            bucket: "influxive".to_string(),
            retention: "72h".to_string(),
            metric_write: InfluxiveWriterConfig::default(),
        }
    }
}

impl InfluxiveChildSvcConfig {
    /// Apply [InfluxiveChildSvcConfig::download_binaries].
    #[cfg(feature = "download_binaries")]
    pub fn with_download_binaries(mut self, download_binaries: bool) -> Self {
        self.download_binaries = download_binaries;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::influxd_path].
    pub fn with_influxd_path(mut self, influxd_path: Option<std::path::PathBuf>) -> Self {
        self.influxd_path = influxd_path;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::influx_path].
    pub fn with_influx_path(mut self, influx_path: Option<std::path::PathBuf>) -> Self {
        self.influx_path = influx_path;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::database_path].
    pub fn with_database_path(mut self, database_path: Option<std::path::PathBuf>) -> Self {
        self.database_path = database_path;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::user].
    pub fn with_user(mut self, user: String) -> Self {
        self.user = user;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::pass].
    pub fn with_pass(mut self, pass: String) -> Self {
        self.pass = pass;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::org].
    pub fn with_org(mut self, org: String) -> Self {
        self.org = org;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::bucket].
    pub fn with_bucket(mut self, bucket: String) -> Self {
        self.bucket = bucket;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::retention].
    pub fn with_retention(mut self, retention: String) -> Self {
        self.retention = retention;
        self
    }

    /// Apply [InfluxiveChildSvcConfig::metric_write].
    pub fn with_metric_write(mut self, metric_write: InfluxiveWriterConfig) -> Self {
        self.metric_write = metric_write;
        self
    }
}

/// A running child-process instance of influxd.
/// Command and control functions are provided through the influx cli tool.
/// Metric writing is provided through the http line protocol.
pub struct InfluxiveChildSvc {
    config: InfluxiveChildSvcConfig,
    host: String,
    token: String,
    child: std::sync::Mutex<Option<tokio::process::Child>>,
    influx_path: std::path::PathBuf,
    writer: InfluxiveWriter,
}

impl InfluxiveChildSvc {
    /// Spawn a new influxd child process.
    pub async fn new(config: InfluxiveChildSvcConfig) -> Result<Self> {
        let db_path = config.database_path.clone().unwrap_or_else(|| {
            let mut db_path = std::path::PathBuf::from(".");
            db_path.push("influxive");
            db_path
        });

        tokio::fs::create_dir_all(&db_path).await?;

        let influxd_path = validate_influx(&db_path, &config, false).await?;

        let influx_path = validate_influx(&db_path, &config, true).await?;

        let (child, port) = spawn_influxd(&db_path, &influxd_path).await?;

        let host = format!("http://127.0.0.1:{port}");

        let mut configs_path = std::path::PathBuf::from(&db_path);
        configs_path.push("configs");

        if let Err(err) = cmd_output!(
            &influx_path,
            "setup",
            "--json",
            "--configs-path",
            &configs_path,
            "--host",
            &host,
            "--username",
            &config.user,
            "--password",
            &config.pass,
            "--org",
            &config.org,
            "--bucket",
            &config.bucket,
            "--retention",
            &config.retention,
            "--force"
        ) {
            let repr = format!("{err:?}");
            if !repr.contains("Error: instance has already been set up") {
                return Err(err);
            }
        }

        let token = tokio::fs::read(&configs_path).await?;
        let token = String::from_utf8_lossy(&token);
        let mut token = token.split("token = \"");
        token.next().unwrap();
        let token = token.next().unwrap();
        let mut token = token.split('\"');
        let token = token.next().unwrap().to_string();

        let writer = InfluxiveWriter::with_token_auth(
            config.metric_write.clone(),
            &host,
            &config.bucket,
            &token,
        );

        let bucket = config.bucket.clone();

        let this = Self {
            config,
            host,
            token,
            child: std::sync::Mutex::new(Some(child)),
            influx_path,
            writer,
        };

        let mut millis = 10;

        for _ in 0..10 {
            // this ensures the db / bucket is created + ready to go
            this.write_metric(
                Metric::new(std::time::SystemTime::now(), "influxive.start")
                    .with_field("value", true),
            );

            if let Ok(result) = this
                .query(format!(
                    r#"from(bucket: "{bucket}")
|> range(start: -15m, stop: now())
|> filter(fn: (r) => r["_measurement"] == "influxive.start")
|> filter(fn: (r) => r["_field"] == "value")"#,
                ))
                .await
            {
                if result.split('\n').count() >= 3 {
                    return Ok(this);
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
            millis *= 2;
        }

        Err(err_other("Unable to start influxd"))
    }

    /// Shut down the child process. Further calls to it should error.
    pub fn shutdown(&self) {
        drop(self.child.lock().unwrap().take());
    }

    /// Get the config this instance was created with.
    pub fn get_config(&self) -> &InfluxiveChildSvcConfig {
        &self.config
    }

    /// Get the host url of this running influxd instance.
    pub fn get_host(&self) -> &str {
        &self.host
    }

    /// Get the operator token of this running influxd instance.
    pub fn get_token(&self) -> &str {
        &self.token
    }

    /// "Ping" the running InfluxDB instance, returning the result.
    pub async fn ping(&self) -> Result<()> {
        cmd_output!(&self.influx_path, "ping", "--host", &self.host)?;
        Ok(())
    }

    /// Run a query against the running InfluxDB instance.
    /// Note, if you are writing metrics, prefer the 'write_metric' api
    /// as that will be more efficient.
    pub async fn query<Q: Into<StringType>>(&self, flux_query: Q) -> Result<String> {
        cmd_output!(
            &self.influx_path,
            "query",
            "--raw",
            "--org",
            &self.config.org,
            "--host",
            &self.host,
            "--token",
            &self.token,
            flux_query.into().into_string()
        )
    }

    /// List the existing dashboard data in the running InfluxDB instance.
    pub async fn list_dashboards(&self) -> Result<String> {
        cmd_output!(
            &self.influx_path,
            "dashboards",
            "--org",
            &self.config.org,
            "--host",
            &self.host,
            "--token",
            &self.token,
            "--json"
        )
    }

    /// Apply a template to the running InfluxDB instance.
    pub async fn apply(&self, template: &[u8]) -> Result<String> {
        use tokio::io::AsyncWriteExt;

        let (file, tmp) = tempfile::Builder::new()
            .suffix(".json")
            .tempfile()?
            .into_parts();
        let mut file = tokio::fs::File::from_std(file);

        file.write_all(template).await?;
        file.shutdown().await?;

        let result = cmd_output!(
            &self.influx_path,
            "apply",
            "--org",
            &self.config.org,
            "--host",
            &self.host,
            "--token",
            &self.token,
            "--json",
            "--force",
            "yes",
            "--file",
            &tmp
        );

        drop(file);

        // Okay if this fails on windows
        let _ = tmp.close();

        result
    }

    /// Log a metric to the running InfluxDB instance.
    /// Note, this function itself is an efficiency abstraction,
    /// which will return quickly if there is space in the buffer.
    /// The actual call to log the metrics will be made a configurable
    /// timespan later to facilitate batching of metric writes.
    pub fn write_metric(&self, metric: Metric) {
        self.writer.write_metric(metric);
    }
}

impl MetricWriter for InfluxiveChildSvc {
    fn write_metric(&self, metric: Metric) {
        InfluxiveChildSvc::write_metric(self, metric);
    }
}

#[cfg(feature = "download_binaries")]
async fn dl_influx(
    _db_path: &std::path::Path,
    is_cli: bool,
    bin_path: &mut std::path::PathBuf,
    err_list: &mut Vec<std::io::Error>,
) -> Option<String> {
    let spec = if is_cli {
        &download_binaries::DL_CLI
    } else {
        &download_binaries::DL_DB
    };

    if let Some(spec) = &spec {
        match spec.download(_db_path).await {
            Ok(path) => {
                *bin_path = path;
                match cmd_output!(&bin_path, "version") {
                    Ok(ver) => return Some(ver),
                    Err(err) => {
                        err_list.push(err_other(format!("failed to run {bin_path:?}")));
                        err_list.push(err);
                    }
                }
            }
            Err(err) => {
                err_list.push(err_other("failed to download"));
                err_list.push(err);
            }
        }
    } else {
        err_list.push(err_other("no download configured for this target os/arch"));
    }

    None
}

async fn validate_influx(
    _db_path: &std::path::Path,
    config: &InfluxiveChildSvcConfig,
    is_cli: bool,
) -> Result<std::path::PathBuf> {
    let mut bin_path = if is_cli {
        "influx".into()
    } else {
        "influxd".into()
    };

    if is_cli {
        if let Some(path) = &config.influx_path {
            bin_path = path.clone();
        }
    } else if let Some(path) = &config.influxd_path {
        bin_path = path.clone();
    };

    let ver = match cmd_output!(&bin_path, "version") {
        Ok(ver) => ver,
        Err(err) => {
            let mut err_list = Vec::new();
            err_list.push(err_other(format!("failed to run {bin_path:?}")));
            err_list.push(err);

            #[cfg(feature = "download_binaries")]
            {
                if let Some(ver) = dl_influx(_db_path, is_cli, &mut bin_path, &mut err_list).await {
                    ver
                } else {
                    return Err(err_other(format!("{err_list:?}",)));
                }
            }

            #[cfg(not(feature = "download_binaries"))]
            {
                return Err(err_other(format!("{err_list:?}",)));
            }
        }
    };

    // alas, the cli prints out the unhelpful version "dev".
    if is_cli && !ver.contains("build_date: 2023-04-28") {
        return Err(err_other(format!("invalid build_date: {ver}")));
    } else if !is_cli && !ver.contains("InfluxDB v2.7.6") {
        return Err(err_other(format!("invalid version: {ver}")));
    }

    Ok(bin_path)
}

async fn spawn_influxd(
    db_path: &std::path::Path,
    influxd_path: &std::path::Path,
) -> Result<(tokio::process::Child, u16)> {
    use tokio::io::AsyncBufReadExt;

    let (s, r) = tokio::sync::oneshot::channel();

    let mut s = Some(s);

    let mut engine_path = std::path::PathBuf::from(db_path);
    engine_path.push("engine");
    let mut bolt_path = std::path::PathBuf::from(db_path);
    bolt_path.push("influxd.bolt");

    let mut proc = tokio::process::Command::new(influxd_path);
    proc.kill_on_drop(true);
    proc.arg("--engine-path").arg(engine_path);
    proc.arg("--bolt-path").arg(bolt_path);
    proc.arg("--http-bind-address").arg("127.0.0.1:0");
    proc.arg("--metrics-disabled");
    proc.arg("--reporting-disabled");
    proc.stdout(std::process::Stdio::piped());

    let proc_err = format!("{proc:?}");

    let mut child = proc
        .spawn()
        .map_err(|err| err_other(format!("{proc_err}: {err:?}")))?;

    let stdout = child.stdout.take().unwrap();
    let mut reader = tokio::io::BufReader::new(stdout).lines();

    tokio::task::spawn(async move {
        while let Some(line) = reader.next_line().await.expect("could not get next line") {
            tracing::trace!(?line, "influxd stdout");
            if line.contains("msg=Listening")
                && line.contains("service=tcp-listener")
                && line.contains("transport=http")
            {
                let mut iter = line.split(" port=");
                iter.next().unwrap();
                let item = iter.next().unwrap();
                let port: u16 = item.parse().unwrap();
                if let Some(s) = s.take() {
                    let _ = s.send(port);
                }
            }
        }
    });

    let port = r.await.map_err(|_| err_other("Failed to scrape port"))?;

    Ok((child, port))
}

#[cfg(test)]
mod test;
