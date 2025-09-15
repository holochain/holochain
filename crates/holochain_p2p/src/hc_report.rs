//! If enabled in the holochain conductor config, this module
//! will collect reports when fetching data from remote peers,
//! aggregate and sign those reports and forward them to those peers
//! who will write the reports to a file on disk.

use holochain_types::report::{ReportEntry, ReportEntryFetchedOps};
use kitsune2_api::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// Accept reports within 5 minutes (microseconds).
const REPORT_WINDOW_US: i64 = 1000 * 1000 * 60 * 5;

const MOD: &str = "hcReport";

/// HcReport configuration types.
mod config {
    /// Configuration parameters for [HcReportFactory](super::HcReportFactory).
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct HcReportConfig {
        /// How many days worth of report files to retain.
        pub days_retained: u32,

        /// Directory path for the report files.
        ///
        /// Files will be named `hc-report.YYYY-MM-DD.jsonl`.
        pub path: std::path::PathBuf,

        /// How often to report Fetched-Op aggregated data in seconds.
        pub fetched_op_interval_s: u32,
    }

    impl Default for HcReportConfig {
        fn default() -> Self {
            Self {
                days_retained: 5,
                // This is always provided by holochain, placing the reports
                // in `<data-root>/reports` directory. Tests specify a
                // tempdir here.
                path: "/tmp/hc-report".into(),
                fetched_op_interval_s: 60,
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

pub use config::*;

/// Holochain report module. See module level docs for details.
pub struct HcReportFactory {
    lair_client: holochain_keystore::MetaLairClient,

    #[cfg(test)]
    test_instance: Mutex<Option<Arc<HcReport>>>,
}

impl std::fmt::Debug for HcReportFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HcReportFactory").finish()
    }
}

impl HcReportFactory {
    /// Construct a new [`HcReportFactory`]
    pub fn create(lair_client: holochain_keystore::MetaLairClient) -> DynReportFactory {
        let out: DynReportFactory = Arc::new(Self {
            lair_client,

            #[cfg(test)]
            test_instance: Mutex::new(None),
        });
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
        let out: K2Result<DynReport> = (|| {
            let lair_client = self.lair_client.clone();
            let config: HcReportModConfig = builder.config.get_module_config()?;
            let out = HcReport::create(config.hc_report, tx, lair_client)?;

            #[cfg(test)]
            {
                *self.test_instance.lock().unwrap() = Some(out.clone());
            }

            let out: DynReport = out;
            Ok(out)
        })();

        Box::pin(async move { out })
    }
}

/// Type sent on our internal command channel.
enum Cmd {
    /// Indicates we have received op data from a remote peer.
    FetchedOp {
        space_id: SpaceId,
        source: Url,
        size_bytes: u64,
    },
}

struct HcReport {
    /// Weak self reference.
    this: Weak<Self>,

    /// Lair client for cryptography.
    lair_client: holochain_keystore::MetaLairClient,

    /// Timing loop task.
    task: tokio::task::AbortHandle,

    /// Map of LocalAgentStores letting us sign reports.
    spaces: Mutex<HashMap<SpaceId, DynLocalAgentStore>>,

    /// Sender side of command channel.
    cmd_send: tokio::sync::mpsc::Sender<Cmd>,

    /// Receiver side of command channel.
    cmd_recv: Mutex<tokio::sync::mpsc::Receiver<Cmd>>,

    /// Transport for sending messages to remote peers.
    tx: DynTransport,

    /// The log file writer.
    file_writer: Mutex<tracing_appender::non_blocking::NonBlocking>,

    /// The guard that shuts down the non-blocking task on drop.
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
        let this = self.this.clone();

        tokio::task::spawn(async move {
            let report: holochain_types::report::ReportEntry = match serde_json::from_slice(&data) {
                Ok(r) => r,
                Err(err) => {
                    tracing::warn!(?err, "received unparsable report entry");
                    return;
                }
            };

            let report = match report {
                holochain_types::report::ReportEntry::FetchedOps(report) => report,
                _ => {
                    tracing::debug!("ignoring non-fetched-ops report");
                    return;
                }
            };

            let timestamp: i64 = match report.timestamp.parse() {
                Ok(t) => t,
                Err(_) => {
                    tracing::warn!("invalid report timestamp");
                    return;
                }
            };
            let now = Timestamp::now().as_micros();
            let diff = if now >= timestamp {
                now - timestamp
            } else {
                timestamp - now
            };
            if diff > REPORT_WINDOW_US {
                tracing::warn!("ignoring received fetch op report outside of reporting window");
                return;
            }

            if !report.verify().await {
                tracing::warn!("ignoring received fetch op report with invalid signatures");
                return;
            }

            // on receiving a report, write it to our report file
            if let Some(this) = this.upgrade() {
                this.write_bytes(&data);
            } else {
                tracing::warn!("Cannot write fetched ops report, module dropped");
            }
        });

        Ok(())
    }
}

impl HcReport {
    pub fn create(
        config: HcReportConfig,
        tx: DynTransport,
        lair_client: holochain_keystore::MetaLairClient,
    ) -> K2Result<Arc<HcReport>> {
        // we're not hooking this up to the tracing library,
        // just using it for the log rotation functionality
        let file = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .max_log_files(config.days_retained as usize)
            .filename_prefix("hc-report")
            .filename_suffix("jsonl")
            .build(config.path)
            .map_err(K2Error::other)?;

        // use it in non-blocking mode so we can call it within async functions
        let (file_writer, _file_guard) = tracing_appender::non_blocking(file);

        let (cmd_send, cmd_recv) = tokio::sync::mpsc::channel(4096);

        let spaces: Mutex<HashMap<SpaceId, DynLocalAgentStore>> = Mutex::new(HashMap::new());

        let out = Arc::new_cyclic(move |this: &Weak<Self>| {
            let freq = config.fetched_op_interval_s;
            let this2 = this.clone();

            // this task will periodically invoke the process_reports()
            // function at the configured frequency
            let task = tokio::task::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(freq as u64)).await;

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
                lair_client,
                task,
                spaces,
                cmd_send,
                cmd_recv: Mutex::new(cmd_recv),
                tx,
                file_writer: Mutex::new(file_writer),
                _file_guard,
            }
        });

        // write a start entry indicating holochain has started
        out.write(ReportEntry::start());

        Ok(out)
    }

    /// Lowest-level write function, actually calls write on the writer.
    fn write_raw(&self, b: &[u8]) {
        if let Err(err) = std::io::Write::write_all(&mut *self.file_writer.lock().unwrap(), b) {
            tracing::error!(?err, "failed to write data to hc-report writer");
        }
    }

    /// Write bytes to the file.
    fn write_bytes(&self, b: &[u8]) {
        let mut out = Vec::with_capacity(b.len() + 1);
        out.extend_from_slice(b);
        out.push(b'\n');
        self.write_raw(&out);
    }

    /// Encode a report entry, and write it to the file.
    fn write(&self, data: ReportEntry) {
        let mut data = match serde_json::to_string(&data) {
            Ok(data) => data,
            Err(err) => {
                tracing::error!(?err, "failed to encode report entry");
                return;
            }
        };
        data.push('\n');
        self.write_raw(data.as_bytes());
    }

    /// Aggregate the reports we received during the interval time
    /// and forward them to the remote peer for writing.
    async fn process_reports(&self) {
        let mut fetched_ops: HashMap<(SpaceId, Url), (u64, u64)> = HashMap::new();

        {
            // pull the reports out of our channel
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
            // get the local agent store for the space we are encoding
            let local_agent_store = match self.spaces.lock().unwrap().get(&space_id) {
                Some(s) => s.clone(),
                None => {
                    tracing::warn!(?space_id, "space not found for fetched op reporting");
                    continue;
                }
            };

            // get all the local agents, because we will sign with all of them
            let local_agents = match local_agent_store.get_all().await {
                Ok(a) => a,
                Err(err) => {
                    tracing::warn!(?err, "failed to fetch local agents for fetch op reporting");
                    continue;
                }
            };

            if local_agents.is_empty() {
                // if there are no local agents, the report would be
                // invalid... abort
                tracing::warn!("no local agents, aborting fetch op reporting");
                continue;
            }

            // first, write out the pubkeys
            let mut agent_pubkeys = Vec::with_capacity(local_agents.len());
            for a in local_agents.iter() {
                agent_pubkeys.push(a.agent().to_string());
            }

            // The report format uses all strings so they can
            // be concatonated in order and signed deterministically.
            // (json can lose information with big integers)
            let timestamp = Timestamp::now().as_micros().to_string();
            let space = space_id.to_string();
            let op_count = op_count.to_string();
            let total_bytes = total_bytes.to_string();

            let mut entry = ReportEntryFetchedOps {
                timestamp,
                space,
                op_count,
                total_bytes,
                agent_pubkeys,
                signatures: Vec::with_capacity(local_agents.len()),
            };

            // do the actual concatenation for generating the signatures
            let to_sign = entry.encode_for_verification();

            for a in local_agents.iter() {
                let agent: [u8; 32] = (&a.agent()[..])
                    .try_into()
                    .expect("array conversion failed");

                // generate a signature for each local agent
                let signature = match self
                    .lair_client
                    .lair_client()
                    .sign_by_pub_key(agent.into(), None, to_sign.clone().into())
                    .await
                {
                    Ok(s) => s,
                    Err(err) => {
                        tracing::error!(?err, "failed to sign message for fetch op reporting");
                        return;
                    }
                };

                use base64::Engine;
                entry
                    .signatures
                    .push(base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(*signature));
            }

            // create the report entry
            let entry = ReportEntry::FetchedOps(entry);

            // serialize the report entry
            let entry = serde_json::to_string(&entry).expect("json serialize");

            // forward the report to the op data source
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
    fn space(&self, space_id: SpaceId, local_agent_store: DynLocalAgentStore) {
        if let Some(this) = self.this.upgrade() {
            // keep the local_agent_store so we can sign
            self.spaces
                .lock()
                .unwrap()
                .insert(space_id.clone(), local_agent_store);

            // register to receive reports from remote peers
            self.tx
                .register_module_handler(space_id.clone(), MOD.into(), this);
        }
    }

    fn fetched_op(&self, space_id: SpaceId, source: Url, _op_id: OpId, size_bytes: u64) {
        // send the fetched op notification to our channel,
        // we will aggregate them next time process_reports() is called
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

    pub const TEST_SPACE_ID: SpaceId = SpaceId(Id(bytes::Bytes::from_static(
        b"12345678901234567890123456789012",
    )));

    #[allow(dead_code)]
    struct Test {
        pub dir: tempfile::TempDir,
        pub kitsune: DynKitsune,
        pub space: DynSpace,
        pub local_agent: DynLocalAgent,
        pub url: Url,
        pub test_report: Arc<HcReport>,
    }

    impl Test {
        pub async fn new() -> Self {
            holochain_trace::test_run();

            crate::check_k2_init();

            let keystore = holochain_keystore::test_keystore();

            // unique temp dir per test kitsune instance
            let dir = tempfile::tempdir().unwrap();

            let report_factory = Arc::new(HcReportFactory {
                lair_client: keystore.clone(),
                test_instance: Mutex::new(None),
            });

            // set up the builder with our report factory
            let b = kitsune2_api::Builder {
                report: report_factory.clone(),
                ..kitsune2_core::default_test_builder()
            }
            .with_default_config()
            .unwrap();

            // configure the report factory
            b.config
                .set_module_config(&HcReportModConfig {
                    hc_report: HcReportConfig {
                        days_retained: 5,
                        path: dir.path().into(),
                        // set this really high, and manually
                        // call process reports
                        fetched_op_interval_s: 60 * 60 * 24,
                    },
                })
                .unwrap();

            // build the kitsune instance
            let kitsune = b.build().await.unwrap();

            // set up space handler
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

            // set up kitsune handler
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

            // register the handler
            // (this is where the report instance is created)
            kitsune.register_handler(Arc::new(K)).await.unwrap();

            // get the space
            let space = kitsune.space(TEST_SPACE_ID).await.unwrap();

            let agent = keystore.new_sign_keypair_random().await.unwrap();
            let agent = crate::HolochainP2pLocalAgent::new(agent, DhtArc::FULL, 1, keystore);

            // register a local agent
            let local_agent: DynLocalAgent = Arc::new(agent);
            space.local_agent_join(local_agent.clone()).await.unwrap();

            // get the url
            let url = space.current_url().unwrap();

            // get the report instance
            let test_report = report_factory
                .test_instance
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .clone();

            // return the test instance
            Self {
                dir,
                kitsune,
                space,
                local_agent,
                url,
                test_report,
            }
        }

        /// List report files in the test temp directory.
        pub async fn list_files(&self) -> Vec<std::path::PathBuf> {
            let mut out = Vec::new();
            let mut d = tokio::fs::read_dir(self.dir.path()).await.unwrap();
            while let Ok(Some(e)) = d.next_entry().await {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("hc-report.")
                    && name.ends_with(".jsonl")
                    && e.file_type().await.unwrap().is_file()
                {
                    out.push(self.dir.path().join(e.file_name()));
                }
            }
            out
        }

        /// Decode all entries in the report files in the test temp directory.
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
            out
        }
    }

    /// Generic sanity test to ensure a start entry is written in the report file.
    #[tokio::test(flavor = "multi_thread")]
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

    /// E2e test ensures a report is written on a remote peer
    /// when a fetched op report is generated on the local peer.
    #[tokio::test(flavor = "multi_thread")]
    async fn report_entry_fetched_ops() {
        let test1 = Test::new().await;
        let test2 = Test::new().await;

        test2.kitsune.report().unwrap().fetched_op(
            TEST_SPACE_ID,
            test1.url.clone(),
            bytes::Bytes::from_static(b"fake-op-id").into(),
            10,
        );
        test2.kitsune.report().unwrap().fetched_op(
            TEST_SPACE_ID,
            test1.url.clone(),
            bytes::Bytes::from_static(b"fake-op-id").into(),
            3,
        );

        // trigger the report process explicitly
        test2.test_report.process_reports().await;

        for _ in 0..100 {
            for entry in test1.read_entries().await {
                if let ReportEntry::FetchedOps(rep) = entry {
                    eprintln!("{rep:#?}");
                    if !rep.verify().await {
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
