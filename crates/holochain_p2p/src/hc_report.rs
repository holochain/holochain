use holochain_types::report::{ReportEntry, ReportEntryFetchedOps};
use kitsune2_api::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

const MOD: &str = "HCREPORT";

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

    fn create(
        &self,
        builder: Arc<Builder>,
        tx: DynTransport,
    ) -> BoxFut<'static, K2Result<DynReport>> {
        Box::pin(async move {
            let config: HcReportModConfig = builder.config.get_module_config()?;
            let out: DynReport = HcReport::create(config.hc_report, tx)?;
            Ok(out)
        })
    }
}

enum Cmd {
    FetchedOp {
        space_id: SpaceId,
        source: Url,
        size_bytes: u64,
    },
}

struct HcReport {
    this: Weak<Self>,
    task: tokio::task::AbortHandle,
    spaces: Arc<Mutex<HashMap<SpaceId, DynLocalAgentStore>>>,
    cmd_send: tokio::sync::mpsc::Sender<Cmd>,
    tx: DynTransport,
    file_writer: Mutex<tracing_appender::non_blocking::NonBlocking>,
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
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

impl TxBaseHandler for HcReport {}

impl TxModuleHandler for HcReport {
    fn recv_module_msg(
        &self,
        _peer: Url,
        _space_id: SpaceId,
        _module: String,
        data: bytes::Bytes,
    ) -> K2Result<()> {
        self.write_bytes(&data);
        Ok(())
    }
}

impl HcReport {
    pub fn create(config: HcReportConfig, tx: DynTransport) -> K2Result<DynReport> {
        let file = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .max_log_files(config.days_retained as usize)
            .filename_prefix("hc-report")
            .filename_suffix("jsonl")
            .build(config.path)
            .map_err(K2Error::other)?;

        let (file_writer, _file_guard) = tracing_appender::non_blocking(file);

        let (cmd_send, mut cmd_recv) = tokio::sync::mpsc::channel(4096);

        let spaces: Arc<Mutex<HashMap<SpaceId, DynLocalAgentStore>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let spaces2 = spaces.clone();
        let tx2 = tx.clone();

        let task = tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs_f64(
                    config.fetched_op_interval_s,
                ))
                .await;

                let mut fetched_ops: HashMap<(SpaceId, Url), (u64, u64)> = HashMap::new();

                while let Ok(cmd) = cmd_recv.try_recv() {
                    match cmd {
                        Cmd::FetchedOp {
                            space_id,
                            source,
                            size_bytes,
                        } => {
                            let e = fetched_ops.entry((space_id, source)).or_default();
                            e.0 += 1;
                            e.1 += size_bytes;
                        }
                    }
                }

                if fetched_ops.is_empty() {
                    continue;
                }

                for ((space_id, source), (op_count, total_bytes)) in fetched_ops {
                    let local_agent_store = match spaces2.lock().unwrap().get(&space_id) {
                        Some(s) => s.clone(),
                        None => {
                            tracing::warn!(?space_id, "space not found for fetched op reporting");
                            continue;
                        }
                    };

                    let local_agents = match local_agent_store.get_all().await {
                        Ok(a) => a,
                        Err(err) => {
                            tracing::warn!(
                                ?err,
                                "failed to fetch local agents for fetch op reporting"
                            );
                            continue;
                        }
                    };

                    let mut agent_pubkeys = Vec::with_capacity(local_agents.len());
                    for a in local_agents.iter() {
                        agent_pubkeys.push(a.agent().to_string());
                    }

                    let mut signatures = Vec::with_capacity(local_agents.len());

                    for _a in local_agents.iter() {
                        //TODO
                        signatures.push("test-sig".to_string());
                    }

                    let entry = ReportEntry::FetchedOps(ReportEntryFetchedOps {
                        timestamp: Timestamp::now().as_micros().to_string(),
                        space: space_id.to_string(),
                        op_count: op_count.to_string(),
                        total_bytes: total_bytes.to_string(),
                        agent_pubkeys,
                        signatures,
                    });

                    let entry = serde_json::to_string(&entry).expect("json serialize");

                    if let Err(err) = tx2
                        .send_module(source, space_id, MOD.into(), entry.into())
                        .await
                    {
                        tracing::warn!(?err, "failed to send fetched ops report to remote peer");
                        continue;
                    }
                }
            }
        })
        .abort_handle();

        let out = Arc::new_cyclic(move |this| HcReport {
            this: this.clone(),
            task,
            spaces,
            cmd_send,
            tx,
            file_writer: Mutex::new(file_writer),
            _file_guard,
        });

        out.write(ReportEntry::start());

        Ok(out)
    }

    fn write_raw(&self, b: &[u8]) {
        if let Err(err) = std::io::Write::write_all(&mut *self.file_writer.lock().unwrap(), b) {
            tracing::error!(?err, "failed to write data to hc-report writer");
        }
    }

    fn write_bytes(&self, b: &[u8]) {
        let mut out = Vec::with_capacity(b.len() + 1);
        out.extend_from_slice(b);
        out.push(b'\n');
        self.write_raw(&out);
    }

    fn write(&self, data: ReportEntry) {
        let mut data = match serde_json::to_string(&data) {
            Ok(data) => data,
            Err(err) => {
                tracing::error!(?err, "failed to encode report entry");
                return;
            }
        };
        data.push_str("\n");
        self.write_raw(data.as_bytes());
    }
}

impl Report for HcReport {
    fn space(&self, space_id: SpaceId, local_agent_store: DynLocalAgentStore) {
        if let Some(this) = self.this.upgrade() {
            self.spaces
                .lock()
                .unwrap()
                .insert(space_id.clone(), local_agent_store);
            self.tx
                .register_module_handler(space_id.clone(), MOD.into(), this);
        }
    }

    fn fetched_op(&self, space_id: SpaceId, source: Url, _op_id: OpId, size_bytes: u64) {
        if let Err(err) = self.cmd_send.try_send(Cmd::FetchedOp {
            space_id,
            source,
            size_bytes,
        }) {
            tracing::warn!(?err, "failed to process fetched op report");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn report_entry_start() {
        let dir = tempfile::tempdir().unwrap();

        let b = kitsune2_api::Builder {
            report: HcReportFactory::create(),
            ..kitsune2_core::default_test_builder()
        }
        .with_default_config()
        .unwrap();

        b.config
            .set_module_config(&HcReportModConfig {
                hc_report: HcReportConfig {
                    days_retained: 5,
                    path: dir.path().into(),
                    fetched_op_interval_s: 0.001,
                },
            })
            .unwrap();

        let kitsune = b.build().await.unwrap();

        #[derive(Debug)]
        struct H;
        impl KitsuneHandler for H {
            fn create_space(&self, _space_id: SpaceId) -> BoxFut<'_, K2Result<DynSpaceHandler>> {
                unimplemented!()
            }
        }
        kitsune.register_handler(Arc::new(H)).await.unwrap();

        for _ in 0..100 {
            let mut d = tokio::fs::read_dir(dir.path()).await.unwrap();
            while let Ok(Some(e)) = d.next_entry().await {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("hc-report.") && name.ends_with(".jsonl") {
                    let data = tokio::fs::read_to_string(dir.path().join(name))
                        .await
                        .unwrap();
                    if let Some(first_line) = data.split("\n").next() {
                        eprintln!("{first_line}");
                        if let Ok(ReportEntry::Start { .. }) =
                            serde_json::from_str::<ReportEntry>(first_line)
                        {
                            // test pass
                            return;
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        panic!("failed to write report file");
    }
}
