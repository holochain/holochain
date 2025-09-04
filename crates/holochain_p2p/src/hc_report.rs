use kitsune2_api::*;
use std::sync::Arc;

/// HcReport configuration types.
pub mod config {
    /// Configuration parameters for [HcReportFactory](super::HcReportFactory).
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct HcReportConfig {
        /// How many days worth of report files to retain.
        pub days_retained: u32,

        /// Directory path for the report files.
        /// Files will be named `hc-report.YYYY-MM-DD.jsonl`.
        pub path: std::path::PathBuf,

        /// How often to report Fetched-Op aggregated data in seconds.
        pub fetched_op_interval_s: f64,
    }

    impl Default for HcReportConfig {
        fn default() -> Self {
            Self {
                days_retained: 5,
                path: "/tmp/hc-report".into(),
                fetched_op_interval_s: 60.0,
            }
        }
    }

    /// Module-level configuration for HcReport.
    #[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct HcReportModConfig {
        /// HcReport configuration.
        pub hc_report: HcReportConfig,
    }
}

use config::*;

/// A default no-op report module.
#[derive(Debug)]
pub struct HcReportFactory {}

impl HcReportFactory {
    /// Construct a new [`HcReportFactory`]
    pub fn create() -> DynReportFactory {
        let out: DynReportFactory = Arc::new(Self {});
        out
    }
}

impl ReportFactory for HcReportFactory {
    fn default_config(&self, config: &mut Config) -> K2Result<()> {
        config.set_module_config(&HcReportModConfig::default())?;
        Ok(())
    }

    fn validate_config(&self, _config: &Config) -> K2Result<()> {
        Ok(())
    }

    fn create(&self, builder: Arc<Builder>) -> BoxFut<'static, K2Result<DynReport>> {
        Box::pin(async move {
            let config: HcReportModConfig = builder.config.get_module_config()?;
            let out: DynReport = HcReport::create(config.hc_report);
            Ok(out)
        })
    }
}

struct HcReport {
    task: tokio::task::AbortHandle,
}

impl Drop for HcReport {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl std::fmt::Debug for HcReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HcReport").finish()
    }
}

impl HcReport {
    pub fn create(config: HcReportConfig) -> DynReport {
        let task = tokio::task::spawn(async move {
            let file = match tracing_appender::rolling::Builder::new()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .max_log_files(config.days_retained as usize)
                .filename_prefix("hc-report")
                .filename_suffix("jsonl")
                .build(config.path)
            {
                Ok(file) => file,
                Err(err) => {
                    tracing::error!(?err, "failed to create hc-report writer");
                    return;
                }
            };

            let (mut file, _guard) = tracing_appender::non_blocking(file);

            let mut write = move |data: &str| {
                if let Err(err) = std::io::Write::write_all(&mut file, data.as_bytes()) {
                    tracing::error!(?err, "failed to write data to hc-report writer");
                }
            };

            write(r#"{"t":"open"}\n"#);
        })
        .abort_handle();

        let out: DynReport = Arc::new(HcReport { task });
        out
    }
}

impl Report for HcReport {}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn report_file_naming() {
        let dir = tempfile::tempdir().unwrap();

        let mut b = kitsune2_api::Builder {
            ..kitsune2_core::default_test_builder()
        };

        b.report = HcReportFactory::create();

        b.config
            .set_module_config(&HcReportModConfig {
                hc_report: HcReportConfig {
                    days_retained: 5,
                    path: dir.path().into(),
                    fetched_op_interval_s: 0.001,
                },
            })
            .unwrap();

        let b = Arc::new(b);

        let _r = b.report.create(b.clone()).await.unwrap();

        for _ in 0..10 {
            let mut d = tokio::fs::read_dir(dir.path()).await.unwrap();
            while let Ok(Some(e)) = d.next_entry().await {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("hc-report.") && name.ends_with(".jsonl") {
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        panic!("failed to write report file");
    }
}
