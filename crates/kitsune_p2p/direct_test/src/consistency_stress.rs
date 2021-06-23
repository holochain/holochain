//! bunch of nodes gossip consistency test

use kitsune_p2p_direct::dependencies::*;
use kitsune_p2p_direct::prelude::*;
use kitsune_p2p_types::tx2::tx2_utils::*;

use futures::future::{BoxFuture, FutureExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

/// configuration for consistency_stress test
pub struct Config {
    /// how many nodes to create
    pub node_count: usize,

    /// how many agents to join on each node
    pub agents_per_node: usize,
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
                bootstrap_url,
                proxy_url,
            } => {
                write!(
                    f,
                    "{:.4}s: TestStarted {} agents per {} nodes bootstrap:{} proxy:{}",
                    run_time_s, agents_per_node, node_count, bootstrap_url, proxy_url,
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
    observability::test_run().ok();

    let (p_send, p_recv) = futures::channel::mpsc::channel(1024);
    let shutdown = || {
        async move {
            // TODO - make this actually clean up the test somehow
        }
        .boxed()
    };

    tokio::task::spawn(test(config.node_count, config.agents_per_node, p_send));

    (p_recv, shutdown)
}

// -- private -- //

async fn test(
    node_count: usize,
    agents_per_node: usize,
    mut p_send: futures::channel::mpsc::Sender<Progress>,
) {
    kitsune_p2p_types::metrics::init_sys_info_poll();

    let mut p_send_clone = p_send.clone();
    tokio::task::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            let sys_info = kitsune_p2p_types::metrics::get_sys_info();

            let used_mem_gb = sys_info.used_mem_kb as f64 / 1024.0 / 1024.0;
            let cpu_usage_pct = sys_info.proc_cpu_usage_pct_1000 as f64 / 1000.0;
            let net_kb_per_s =
                (sys_info.tx_bytes_per_sec as f64 + sys_info.rx_bytes_per_sec as f64) / 1024.0;

            p_send_clone
                .send(Progress::SysMetrics {
                    used_mem_gb,
                    cpu_usage_pct,
                    net_kb_per_s,
                })
                .await
                .unwrap();
        }
    });

    let test_start = std::time::Instant::now();

    let (bootstrap_url, driver, _bootstrap_close) = new_quick_bootstrap_v1().await.unwrap();
    tokio::task::spawn(driver);

    let (proxy_url, driver, _proxy_close) = new_quick_proxy_v1().await.unwrap();
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

    #[allow(dead_code)]
    struct TestNode {
        kdirect: KitsuneDirect,
        kdhnd: KdHnd,
        agents: Vec<KdHash>,
    }
    let mut nodes = Vec::new();

    for _ in 0..node_count {
        let persist = new_persist_mem();
        let conf = KitsuneDirectV1Config {
            persist,
            bootstrap: bootstrap_url.clone(),
            proxy: proxy_url.clone(),
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

        let mut agents = Vec::new();

        for _ in 0..agents_per_node {
            let agent = kdirect
                .get_persist()
                .generate_signing_keypair()
                .await
                .unwrap();
            kdhnd.app_join(root.clone(), agent.clone()).await.unwrap();

            // sneak this directly into the db : )
            kdirect
                .get_persist()
                .store_entry(root.clone(), agent.clone(), app_entry.clone())
                .await
                .unwrap();

            agents.push(agent);
        }

        nodes.push(TestNode {
            kdirect,
            kdhnd,
            agents,
        });
    }

    p_send
        .send(Progress::TestStarted {
            run_time_s: test_start.elapsed().as_secs_f64(),
            node_count,
            agents_per_node,
            bootstrap_url,
            proxy_url,
        })
        .await
        .unwrap();

    // this loop waits for agent info to be synced
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let target_agent_count = node_count * agents_per_node;
        let mut avg_agent_count = 0;

        for node in nodes.iter() {
            avg_agent_count += node
                .kdirect
                .get_persist()
                .query_agent_info(root.clone())
                .await
                .unwrap()
                .len();
        }
        avg_agent_count /= nodes.len();

        if avg_agent_count >= target_agent_count {
            p_send
                .send(Progress::AgentConsistent {
                    run_time_s: test_start.elapsed().as_secs_f64(),
                    agent_count: avg_agent_count,
                })
                .await
                .unwrap();
            break;
        }

        let target_total_op_count = 0;
        let avg_total_op_count = 0;

        p_send
            .send(Progress::InterimState {
                run_time_s: test_start.elapsed().as_secs_f64(),
                round_elapsed_s: test_start.elapsed().as_secs_f64(),
                target_agent_count,
                avg_agent_count,
                target_total_op_count,
                avg_total_op_count,
            })
            .await
            .unwrap();
    }

    let mut target_total_op_count = 1; // 1 to account for the app_entry

    // this loop publishes ops, and waits for them to be synced
    loop {
        let round_start_time = std::time::Instant::now();

        for node in nodes.iter() {
            for agent in node.agents.iter() {
                node.kdhnd
                    .entry_author(
                        root.clone(),
                        agent.clone(),
                        KdEntryContent {
                            kind: "u.foo".to_string(),
                            parent: app_entry_hash.clone(),
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
                target_total_op_count += 1;
            }
        }

        // this loop waits for the target op count to reach consistency
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            let target_agent_count = node_count * agents_per_node;
            let mut avg_agent_count = 0;

            for node in nodes.iter() {
                avg_agent_count += node
                    .kdirect
                    .get_persist()
                    .query_agent_info(root.clone())
                    .await
                    .unwrap()
                    .len();
            }
            avg_agent_count /= nodes.len();

            let mut avg_total_op_count = 0;

            for node in nodes.iter() {
                for agent in node.agents.iter() {
                    avg_total_op_count += node
                        .kdirect
                        .get_persist()
                        .query_entries(
                            root.clone(),
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
            avg_total_op_count /= target_agent_count;

            if avg_total_op_count >= target_total_op_count {
                p_send
                    .send(Progress::OpConsistent {
                        run_time_s: test_start.elapsed().as_secs_f64(),
                        round_elapsed_s: round_start_time.elapsed().as_secs_f64(),
                        new_ops_added_count: target_agent_count,
                        total_op_count: avg_total_op_count,
                    })
                    .await
                    .unwrap();
                break;
            }

            p_send
                .send(Progress::InterimState {
                    run_time_s: test_start.elapsed().as_secs_f64(),
                    round_elapsed_s: round_start_time.elapsed().as_secs_f64(),
                    target_agent_count,
                    avg_agent_count,
                    target_total_op_count,
                    avg_total_op_count,
                })
                .await
                .unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn consistency_test() {
        let (mut progress, shutdown) = run(Config {
            node_count: 2,
            agents_per_node: 2,
        });

        let deadline = tokio::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(5))
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
