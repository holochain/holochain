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
    spaces: Mutex<HashMap<SpaceId, DynLocalAgentStore>>,
    cmd_send: tokio::sync::mpsc::Sender<Cmd>,
    cmd_recv: Mutex<tokio::sync::mpsc::Receiver<Cmd>>,
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

        let (cmd_send, cmd_recv) = tokio::sync::mpsc::channel(4096);

        let spaces: Mutex<HashMap<SpaceId, DynLocalAgentStore>> = Mutex::new(HashMap::new());

        let out = Arc::new_cyclic(move |this: &Weak<Self>| {
            let freq = config.fetched_op_interval_s;
            let this2 = this.clone();
            let task = tokio::task::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs_f64(freq)).await;

                    if let Some(this) = this2.upgrade() {
                        this.process_reports().await;
                    } else {
                        tracing::debug!("hc report loop ending");
                        return;
                    }
                }
            })
            .abort_handle();

            HcReport {
                this: this.clone(),
                task,
                spaces,
                cmd_send,
                cmd_recv: Mutex::new(cmd_recv),
                tx,
                file_writer: Mutex::new(file_writer),
                _file_guard,
            }
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

    async fn process_reports(&self) {
        let mut fetched_ops: HashMap<(SpaceId, Url), (u64, u64)> = HashMap::new();

        {
            let mut lock = self.cmd_recv.lock().unwrap();
            while let Ok(cmd) = lock.try_recv() {
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
        }

        if fetched_ops.is_empty() {
            // nothing to do
            return;
        }

        for ((space_id, source), (op_count, total_bytes)) in fetched_ops {
            let local_agent_store = match self.spaces.lock().unwrap().get(&space_id) {
                Some(s) => s.clone(),
                None => {
                    tracing::warn!(?space_id, "space not found for fetched op reporting");
                    continue;
                }
            };

            let local_agents = match local_agent_store.get_all().await {
                Ok(a) => a,
                Err(err) => {
                    tracing::warn!(?err, "failed to fetch local agents for fetch op reporting");
                    continue;
                }
            };

            let mut agent_pubkeys = Vec::with_capacity(local_agents.len());
            for a in local_agents.iter() {
                agent_pubkeys.push(a.agent().to_string());
            }

            let timestamp = Timestamp::now().as_micros().to_string();
            let space = space_id.to_string();
            let op_count = op_count.to_string();
            let total_bytes = total_bytes.to_string();

            let mut to_sign = Vec::new();
            to_sign.extend_from_slice(timestamp.as_bytes());
            to_sign.extend_from_slice(space.as_bytes());
            to_sign.extend_from_slice(op_count.as_bytes());
            to_sign.extend_from_slice(total_bytes.as_bytes());
            for a in agent_pubkeys.iter() {
                to_sign.extend_from_slice(a.as_bytes());
            }

            let mut signatures = Vec::with_capacity(local_agents.len());

            const STUB_ID: bytes::Bytes = bytes::Bytes::from_static(b"");
            let stub_ts = Timestamp::now();
            for a in local_agents.iter() {
                let signature = match a.sign(
                    // hc local agent doesn't use any agent info
                    // for signing, so we can use a stub here
                    &AgentInfo {
                        agent: STUB_ID.into(),
                        space: STUB_ID.into(),
                        created_at: stub_ts.into(),
                        expires_at: stub_ts.into(),
                        is_tombstone: false,
                        url: None,
                        storage_arc: DhtArc::default(),
                    },
                    &to_sign,
                ).await {
                    Ok(s) => s,
                    Err(err) => {
                        tracing::warn!(?err, "failed to sign message for fetch op reporting");
                        continue;
                    }
                };

                use base64::Engine;
                signatures.push(base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(signature));
            }

            let entry = ReportEntry::FetchedOps(ReportEntryFetchedOps {
                timestamp,
                space,
                op_count,
                total_bytes,
                agent_pubkeys,
                signatures,
            });

            let entry = serde_json::to_string(&entry).expect("json serialize");

            if let Err(err) = self
                .tx
                .send_module(source, space_id, MOD.into(), entry.into())
                .await
            {
                tracing::warn!(?err, "failed to send fetched ops report to remote peer");
                continue;
            }
        }
    }
}

impl Report for HcReport {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

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

    #[allow(dead_code)]
    struct Test {
        pub dir: tempfile::TempDir,
        pub kitsune: DynKitsune,
        pub space: DynSpace,
        pub local_agent: DynLocalAgent,
        pub url: Url,
        pub verifier: DynVerifier,
    }

    impl Test {
        pub async fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();

            let verifier: DynVerifier = Arc::new(
                kitsune2_test_utils::agent::TestVerifier
            );

            let b = kitsune2_api::Builder {
                verifier: verifier.clone(),
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
                        // set this really high, and manually
                        // call process reports
                        fetched_op_interval_s: 60.0 * 60.0 * 24.0,
                    },
                })
                .unwrap();

            let kitsune = b.build().await.unwrap();

            #[derive(Debug)]
            struct S;
            impl SpaceHandler for S {
                fn recv_notify(
                    &self,
                    _from_peer: Url,
                    _space_id: SpaceId,
                    _data: bytes::Bytes,
                ) -> K2Result<()> {
                    Ok(())
                }
            }

            #[derive(Debug)]
            struct K;
            impl KitsuneHandler for K {
                fn create_space(
                    &self,
                    _space_id: SpaceId,
                ) -> BoxFut<'_, K2Result<DynSpaceHandler>> {
                    let s: DynSpaceHandler = Arc::new(S);
                    Box::pin(async move { Ok(s) })
                }
            }
            kitsune.register_handler(Arc::new(K)).await.unwrap();
            let space = kitsune
                .space(kitsune2_test_utils::space::TEST_SPACE_ID)
                .await
                .unwrap();
            let local_agent: DynLocalAgent =
                Arc::new(kitsune2_test_utils::agent::TestLocalAgent::default());
            space.local_agent_join(local_agent.clone()).await.unwrap();
            let url = space.current_url().unwrap();

            Self {
                dir,
                kitsune,
                space,
                local_agent,
                url,
                verifier,
            }
        }

        pub async fn list_files(&self) -> Vec<std::path::PathBuf> {
            let mut out = Vec::new();
            let mut d = tokio::fs::read_dir(self.dir.path()).await.unwrap();
            while let Ok(Some(e)) = d.next_entry().await {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("hc-report.") && name.ends_with(".jsonl") {
                    if e.file_type().await.unwrap().is_file() {
                        out.push(self.dir.path().join(e.file_name()));
                    }
                }
            }
            out
        }

        pub async fn read_entries(&self) -> Vec<ReportEntry> {
            let mut out = Vec::new();
            for file in self.list_files().await {
                let data = tokio::fs::read_to_string(file).await.unwrap();
                for line in data.split('\n') {
                    let line = line.trim();
                    if !line.is_empty() {
                        if let Ok(entry) = serde_json::from_str::<ReportEntry>(line) {
                            out.push(entry);
                        }
                    }
                }
            }
            return out;
        }
    }

    #[tokio::test]
    async fn report_entry_start() {
        let test = Test::new().await;

        for _ in 0..100 {
            for entry in test.read_entries().await {
                if let ReportEntry::Start(_) = entry {
                    // test pass
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        panic!("failed to write report file");
    }

    #[tokio::test]
    async fn report_entry_fetched_ops() {
        let test1 = Test::new().await;
        let test2 = Test::new().await;

        test2.kitsune.report().unwrap().fetched_op(
            kitsune2_test_utils::space::TEST_SPACE_ID,
            test1.url.clone(),
            bytes::Bytes::from_static(b"fake-op-id").into(),
            10,
        );
        test2.kitsune.report().unwrap().fetched_op(
            kitsune2_test_utils::space::TEST_SPACE_ID,
            test1.url.clone(),
            bytes::Bytes::from_static(b"fake-op-id").into(),
            3,
        );

        test2
            .kitsune
            .report()
            .unwrap()
            .as_any()
            .downcast_ref::<HcReport>()
            .unwrap()
            .process_reports()
            .await;

        for _ in 0..100 {
            for entry in test1.read_entries().await {
                if let ReportEntry::FetchedOps(rep) = entry {
                    eprintln!("{rep:#?}");
                    if !rep.verify(&test1.verifier) {
                        continue;
                    }
                    if rep.op_count == "2" && rep.total_bytes == "13" {
                        // test pass
                        return;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        panic!("failed to write report file");
    }
}
