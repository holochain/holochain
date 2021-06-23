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
    /// The test has started
    TestStarted {
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
        /// the number of agents that all nodes know about
        agent_count: usize,
    },

    /// An op consistency marker has been reached
    OpConsistent {
        /// the number of ops that were synced in this round
        new_ops_added_count: usize,

        /// the total number of ops that all agents know about
        total_op_count: usize,
    },
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
                target_agent_count,
                avg_agent_count,
                target_total_op_count,
                avg_total_op_count,
            })
            .await
            .unwrap();
    }

    let mut target_total_op_count = 0;

    // this loop publishes ops, and waits for them to be synced
    loop {
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
                        new_ops_added_count: target_agent_count,
                        total_op_count: avg_total_op_count,
                    })
                    .await
                    .unwrap();
                break;
            }

            p_send
                .send(Progress::InterimState {
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
            println!("{:?}", progress);
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
