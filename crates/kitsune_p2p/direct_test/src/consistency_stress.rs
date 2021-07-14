//! bunch of nodes gossip consistency test

use kitsune_p2p_direct::dependencies::*;
use kitsune_p2p_direct::prelude::*;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::tx2::tx2_utils::*;

use futures::future::{BoxFuture, FutureExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

/// configuration for consistency_stress test
pub struct Config {
    /// tuning_params
    pub tuning_params: KitsuneP2pTuningParams,

    /// how many nodes to create
    pub node_count: usize,

    /// how many agents to join on each node
    pub agents_per_node: usize,

    /// introduce this number of bad agent infos every round
    pub bad_agent_count: usize,

    /// have this many agents drop after each op consistency
    /// the same number of new agents will also be added
    pub agent_turnover_count: usize,
}

/// progress emitted by this test
#[derive(Debug)]
pub enum Progress {
    /// System Metrics
    SysMetrics {
        /// Used Memory GiB
        used_mem_gb: f64,

        /// CPU usage %
        cpu_usage_pct: f64,

        /// network KiB per sec
        net_kb_per_s: f64,
    },

    /// The test has started
    TestStarted {
        /// how long the test has been running
        run_time_s: f64,

        /// how many nodes were created
        node_count: usize,

        /// how many agents were joined on each oned
        agents_per_node: usize,

        /// test will introduce bad agent infos
        bad_agent_count: usize,

        /// test will drop / add new agents of this number every round
        agent_turnover_count: usize,

        /// bootstrap_url used
        bootstrap_url: TxUrl,

        /// proxy_url used
        proxy_url: TxUrl,
    },

    /// A periodic interim report to show progress
    InterimState {
        /// how long the test has been running
        run_time_s: f64,

        /// the elapsed time since this round started
        round_elapsed_s: f64,

        /// the current target agent count nodes should know about
        target_agent_count: usize,

        /// the avg agent count nodes know about
        avg_agent_count: usize,

        /// the current target total op_count agents should know about
        target_total_op_count: usize,

        /// the avg total op_count agents know about
        avg_total_op_count: usize,
    },

    /// Initial agent consistency has been reached
    AgentConsistent {
        /// how long the test has been running
        run_time_s: f64,

        /// the number of agents that all nodes know about
        agent_count: usize,
    },

    /// An op consistency marker has been reached
    OpConsistent {
        /// how long the test has been running
        run_time_s: f64,

        /// the elapsed time since this round started
        round_elapsed_s: f64,

        /// the number of ops that were synced in this round
        new_ops_added_count: usize,

        /// the total number of ops that all agents know about
        total_op_count: usize,
    },
}

impl std::fmt::Display for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Progress::SysMetrics {
                used_mem_gb,
                cpu_usage_pct,
                net_kb_per_s,
            } => {
                write!(
                    f,
                    "-- CPU {:.0} % -- MEM {:.3} GiB -- NET {:.3} KiB/s --",
                    cpu_usage_pct, used_mem_gb, net_kb_per_s,
                )
            }
            Progress::TestStarted {
                run_time_s,
                node_count,
                agents_per_node,
                bad_agent_count,
                agent_turnover_count,
                bootstrap_url,
                proxy_url,
            } => {
                write!(
                    f,
                    r#"{:.4}s: TestStarted
 -- {} agents / {} nodes
 -- bad_agent_count: {}
 -- agent_turnover_count: {}
 -- bootstrap: {}
 -- proxy: {}"#,
                    run_time_s,
                    agents_per_node,
                    node_count,
                    bad_agent_count,
                    agent_turnover_count,
                    bootstrap_url,
                    proxy_url,
                )
            }
            Progress::InterimState {
                run_time_s,
                round_elapsed_s,
                target_agent_count,
                avg_agent_count,
                target_total_op_count,
                avg_total_op_count,
            } => {
                write!(
                    f,
                    "{:.4}s: InterimState {:.4}s {}/{} agents {}/{} ops",
                    run_time_s,
                    round_elapsed_s,
                    avg_agent_count,
                    target_agent_count,
                    avg_total_op_count,
                    target_total_op_count,
                )
            }
            Progress::AgentConsistent {
                run_time_s,
                agent_count,
            } => {
                write!(
                    f,
                    "--!!--\n{:.4}s: ! AgentConsistent ! {} agents\n--!!--",
                    run_time_s, agent_count,
                )
            }
            Progress::OpConsistent {
                run_time_s,
                round_elapsed_s,
                new_ops_added_count,
                total_op_count,
            } => {
                write!(
                    f,
                    "--!!--\n{:.4}s: ! OpConsistent ! {:.4}s {} new {} total\n--!!--",
                    run_time_s, round_elapsed_s, new_ops_added_count, total_op_count,
                )
            }
        }
    }
}

/// run the consistency_stress test
pub fn run(
    config: Config,
) -> (
    impl futures::stream::Stream<Item = Progress>,
    impl FnOnce() -> BoxFuture<'static, ()>,
) {
    use tracing_subscriber::{filter::EnvFilter, fmt::time::ChronoUtc, FmtSubscriber};
    tracing::subscriber::set_global_default(
        FmtSubscriber::builder()
            .with_writer(std::io::stderr)
            // like rfc3339 but slightly more terse
            .with_timer(ChronoUtc::with_format("%Y-%m-%dT%H:%M:%S.%3fZ".to_string()))
            .with_env_filter(EnvFilter::from_default_env())
            .pretty()
            .finish(),
    )
    .unwrap();

    let (p_send, p_recv) = futures::channel::mpsc::channel(1024);
    let shutdown = || {
        async move {
            // TODO - make this actually clean up the test somehow
        }
        .boxed()
    };

    tokio::task::spawn(test(
        config.tuning_params,
        config.node_count,
        config.agents_per_node,
        config.bad_agent_count,
        config.agent_turnover_count,
        p_send,
    ));

    (p_recv, shutdown)
}

// -- private -- //

struct TestNode {
    kdirect: KitsuneDirect,
    kdhnd: KdHnd,
    agents: Vec<KdHash>,
}

struct Test {
    node_count: usize,
    agents_per_node: usize,
    bad_agent_count: usize,
    agent_turnover_count: usize,
    tuning_params: KitsuneP2pTuningParams,
    p_send: futures::channel::mpsc::Sender<Progress>,
    bootstrap_url: TxUrl,
    proxy_url: TxUrl,
    root: KdHash,
    app_entry: KdEntrySigned,
    app_entry_hash: KdHash,
    nodes: Vec<TestNode>,

    time_test_start: std::time::Instant,
    #[allow(dead_code)]
    time_round_start: std::time::Instant,

    target_agent_count: usize,
    target_total_op_count: usize,
}

impl Test {
    async fn new(
        node_count: usize,
        agents_per_node: usize,
        bad_agent_count: usize,
        agent_turnover_count: usize,
        tuning_params: KitsuneP2pTuningParams,
        p_send: futures::channel::mpsc::Sender<Progress>,
    ) -> Self {
        let time_test_start = std::time::Instant::now();

        let (bootstrap_url, driver, _bootstrap_close) =
            new_quick_bootstrap_v1(tuning_params.clone()).await.unwrap();
        tokio::task::spawn(driver);

        let (proxy_url, driver, _proxy_close) =
            new_quick_proxy_v1(tuning_params.clone()).await.unwrap();
        tokio::task::spawn(driver);

        let (root, app_entry) = {
            let root_persist = new_persist_mem();
            let root = root_persist.generate_signing_keypair().await.unwrap();
            let app_entry = KdEntryContent {
                kind: "s.app".to_string(),
                parent: root.clone(),
                author: root.clone(),
                verify: "".to_string(),
                data: serde_json::json!({}),
            };
            let app_entry = KdEntrySigned::from_content(&root_persist, app_entry)
                .await
                .unwrap();
            (root, app_entry)
        };

        let app_entry_hash = app_entry.hash().clone();

        Self {
            node_count,
            agents_per_node,
            bad_agent_count,
            agent_turnover_count,
            tuning_params,
            p_send,
            bootstrap_url,
            proxy_url,
            root,
            app_entry,
            app_entry_hash,
            nodes: Vec::new(),

            time_test_start,
            time_round_start: std::time::Instant::now(),

            target_agent_count: node_count * agents_per_node,
            target_total_op_count: 1, // 1 for app_entry
        }
    }

    async fn add_agent_to_node(&mut self, node_idx: usize) {
        let kdirect = self.nodes[node_idx].kdirect.clone();
        let kdhnd = self.nodes[node_idx].kdhnd.clone();

        let agent = kdirect
            .get_persist()
            .generate_signing_keypair()
            .await
            .unwrap();

        kdhnd
            .app_join(self.root.clone(), agent.clone())
            .await
            .unwrap();

        // sneak this directly into the db : )
        kdirect
            .get_persist()
            .store_entry(self.root.clone(), agent.clone(), self.app_entry.clone())
            .await
            .unwrap();

        self.nodes[node_idx].agents.push(agent);
    }

    async fn add_node(&mut self) {
        let persist = new_persist_mem();
        let conf = KitsuneDirectV1Config {
            tuning_params: self.tuning_params.clone(),
            persist,
            bootstrap: self.bootstrap_url.clone(),
            proxy: self.proxy_url.clone(),
            ui_port: 0,
        };

        let (kdirect, driver) = new_kitsune_direct_v1(conf).await.unwrap();
        tokio::task::spawn(driver);

        let (kdhnd, mut evt) = kdirect.bind_control_handle().await.unwrap();
        tokio::task::spawn(async move {
            while let Some(evt) = evt.next().await {
                tracing::trace!(?evt);
            }
        });

        self.nodes.push(TestNode {
            kdirect,
            kdhnd,
            agents: Vec::new(),
        });

        let node_idx = self.nodes.len() - 1;

        for _ in 0..self.agents_per_node {
            self.add_agent_to_node(node_idx).await;
        }
    }

    async fn inject_bad_agent_info(&mut self) {
        for _ in 0..self.bad_agent_count {
            self.target_agent_count += 1;

            let info = gen_bad_agent_info(
                self.tuning_params.clone(),
                self.root.clone(),
                self.bootstrap_url.clone(),
                self.proxy_url.clone(),
            )
            .await;

            for node in self.nodes.iter() {
                node.kdirect
                    .get_persist()
                    .store_agent_info(info.clone())
                    .await
                    .unwrap();
            }
        }
    }

    async fn turnover_agents(&mut self) {
        use rand::Rng;

        for _ in 0..self.agent_turnover_count {
            let node_idx = rand::thread_rng().gen_range(0..self.nodes.len());
            let agent_idx = rand::thread_rng().gen_range(0..self.nodes[node_idx].agents.len());
            let agent = self.nodes[node_idx].agents.remove(agent_idx);
            self.nodes[node_idx]
                .kdhnd
                .app_leave(self.root.clone(), agent)
                .await
                .unwrap();
            self.add_agent_to_node(node_idx).await;
            // we don't remove the old one, because the agent info is
            // still going to be in the db / gossiped
            self.target_agent_count += 1;
            println!("Turned Over Agent in NODE {}", node_idx);
        }
    }

    async fn calc_avgs(&mut self) -> (usize, usize) {
        let mut avg_agent_count = 0;

        for node in self.nodes.iter() {
            avg_agent_count += node
                .kdirect
                .get_persist()
                .query_agent_info(self.root.clone())
                .await
                .unwrap()
                .len();
        }
        avg_agent_count /= self.nodes.len();

        let mut avg_total_op_count = 0;

        for node in self.nodes.iter() {
            for agent in node.agents.iter() {
                avg_total_op_count += node
                    .kdirect
                    .get_persist()
                    .query_entries(
                        self.root.clone(),
                        agent.clone(),
                        f32::MIN,
                        f32::MAX,
                        DhtArc::new(0, u32::MAX),
                    )
                    .await
                    .unwrap()
                    .len();
            }
        }
        avg_total_op_count /= self.node_count * self.agents_per_node;

        (avg_agent_count, avg_total_op_count)
    }

    fn new_round(&mut self) {
        self.time_round_start = std::time::Instant::now();
    }

    async fn emit_test_started(&mut self) {
        self.p_send
            .send(Progress::TestStarted {
                run_time_s: self.time_test_start.elapsed().as_secs_f64(),
                node_count: self.node_count,
                agents_per_node: self.agents_per_node,
                bad_agent_count: self.bad_agent_count,
                agent_turnover_count: self.agent_turnover_count,
                bootstrap_url: self.bootstrap_url.clone(),
                proxy_url: self.proxy_url.clone(),
            })
            .await
            .unwrap();
    }

    async fn emit_interim(&mut self, avg_agent_count: usize, avg_total_op_count: usize) {
        self.p_send
            .send(Progress::InterimState {
                run_time_s: self.time_test_start.elapsed().as_secs_f64(),
                round_elapsed_s: self.time_round_start.elapsed().as_secs_f64(),
                target_agent_count: self.target_agent_count,
                avg_agent_count,
                target_total_op_count: self.target_total_op_count,
                avg_total_op_count,
            })
            .await
            .unwrap();
    }

    async fn emit_agent_consistent(&mut self, agent_count: usize) {
        self.p_send
            .send(Progress::AgentConsistent {
                run_time_s: self.time_test_start.elapsed().as_secs_f64(),
                agent_count,
            })
            .await
            .unwrap();
    }

    async fn emit_op_consistent(&mut self, total_op_count: usize) {
        self.p_send
            .send(Progress::OpConsistent {
                run_time_s: self.time_test_start.elapsed().as_secs_f64(),
                round_elapsed_s: self.time_round_start.elapsed().as_secs_f64(),
                new_ops_added_count: self.node_count * self.agents_per_node,
                total_op_count,
            })
            .await
            .unwrap();
    }
}

async fn test(
    tuning_params: KitsuneP2pTuningParams,
    node_count: usize,
    agents_per_node: usize,
    bad_agent_count: usize,
    agent_turnover_count: usize,
    mut p_send: futures::channel::mpsc::Sender<Progress>,
) {
    kitsune_p2p_types::metrics::init_sys_info_poll();

    let mut test = Test::new(
        node_count,
        agents_per_node,
        bad_agent_count,
        agent_turnover_count,
        tuning_params.clone(),
        p_send.clone(),
    )
    .await;

    tokio::task::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            let sys_info = kitsune_p2p_types::metrics::get_sys_info();

            let used_mem_gb = sys_info.used_mem_kb as f64 / 1024.0 / 1024.0;
            let cpu_usage_pct = sys_info.proc_cpu_usage_pct_1000 as f64 / 1000.0;
            let net_kb_per_s =
                (sys_info.tx_bytes_per_sec as f64 + sys_info.rx_bytes_per_sec as f64) / 1024.0;

            p_send
                .send(Progress::SysMetrics {
                    used_mem_gb,
                    cpu_usage_pct,
                    net_kb_per_s,
                })
                .await
                .unwrap();
        }
    });

    for _ in 0..node_count {
        test.add_node().await;
    }

    test.emit_test_started().await;

    test.inject_bad_agent_info().await;

    // this loop waits for agent info to be synced
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let (avg_agent_count, avg_total_op_count) = test.calc_avgs().await;

        if avg_agent_count >= test.target_agent_count {
            test.emit_agent_consistent(avg_agent_count).await;
            break;
        }

        test.emit_interim(avg_agent_count, avg_total_op_count).await;
    }

    // this loop publishes ops, and waits for them to be synced
    loop {
        test.new_round();

        test.inject_bad_agent_info().await;
        test.turnover_agents().await;

        for node in test.nodes.iter() {
            for agent in node.agents.iter() {
                node.kdhnd
                    .entry_author(
                        test.root.clone(),
                        agent.clone(),
                        KdEntryContent {
                            kind: "u.foo".to_string(),
                            parent: test.app_entry_hash.clone(),
                            author: agent.clone(),
                            verify: "".to_string(),
                            data: serde_json::json!({
                                "nonce": std::time::SystemTime::now()
                                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs_f64(),
                            }),
                        },
                        vec![].into_boxed_slice().into(),
                    )
                    .await
                    .unwrap();
                test.target_total_op_count += 1;
            }
        }

        // this loop waits for the target op count to reach consistency
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            let (avg_agent_count, avg_total_op_count) = test.calc_avgs().await;

            if avg_total_op_count >= test.target_total_op_count {
                test.emit_op_consistent(avg_total_op_count).await;
                break;
            }

            test.emit_interim(avg_agent_count, avg_total_op_count).await;
        }
    }
}

async fn gen_bad_agent_info(
    tuning_params: KitsuneP2pTuningParams,
    root: KdHash,
    bootstrap: TxUrl,
    proxy: TxUrl,
) -> KdAgentInfo {
    let persist = new_persist_mem();
    let conf = KitsuneDirectV1Config {
        tuning_params,
        persist,
        bootstrap,
        proxy,
        ui_port: 0,
    };

    let (kdirect, driver) = new_kitsune_direct_v1(conf).await.unwrap();
    tokio::task::spawn(driver);

    let (kdhnd, mut evt) = kdirect.bind_control_handle().await.unwrap();
    tokio::task::spawn(async move {
        while let Some(evt) = evt.next().await {
            tracing::trace!(?evt);
        }
    });

    let agent = kdirect
        .get_persist()
        .generate_signing_keypair()
        .await
        .unwrap();

    kdhnd.app_join(root.clone(), agent.clone()).await.unwrap();

    let agent_info = kdirect
        .get_persist()
        .get_agent_info(root.clone(), agent)
        .await
        .unwrap();

    kdhnd.close(0, "").await;
    kdirect.close(0, "").await;

    println!("BAD_AGENT_INFO: {:?}", agent_info);
    agent_info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn consistency_test() {
        let mut tuning_params =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning_params.gossip_peer_on_success_next_gossip_delay_ms = 1000;
        let tuning_params = std::sync::Arc::new(tuning_params);
        let (mut progress, shutdown) = run(Config {
            tuning_params,
            node_count: 2,
            agents_per_node: 2,
            bad_agent_count: 0,
            agent_turnover_count: 1,
        });

        let deadline = tokio::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(10))
            .unwrap();

        let mut test_started = false;
        let mut agent_consistent = false;
        let mut op_consistent = false;

        while let Ok(Some(progress)) = tokio::time::timeout_at(deadline, progress.next()).await {
            println!("{}", progress);
            match progress {
                Progress::TestStarted { .. } => test_started = true,
                Progress::AgentConsistent { .. } => agent_consistent = true,
                Progress::OpConsistent { .. } => {
                    op_consistent = true;
                    break;
                }
                _ => (),
            }
        }

        shutdown().await;

        assert!(test_started);
        assert!(agent_consistent);
        assert!(op_consistent);
    }
}
