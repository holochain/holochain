//! Kitsune P2p Direct Application Framework Test Harness
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(unsafe_code)]
#![allow(clippy::blocks_in_if_conditions)]

use futures::future::{BoxFuture, FutureExt};
use futures::stream::StreamExt;
use kitsune_p2p_direct::dependencies::*;
use kitsune_p2p_direct::prelude::*;
use kitsune_p2p_types::metrics::metric_task;
use kitsune_p2p_types::tx2::tx2_utils::*;

/// init tracing
pub fn init_tracing() {
    observability::test_run().ok();
}

/// kdirect version harness specifier
pub enum KdVerSpec {
    /// v1 kdirect impl
    V1,
}

/// response type for agent hook execution
pub type AgentHookResp = BoxFuture<'static, KdResult<()>>;

/// input parameter type for agent hook execution
pub struct AgentHookInput {
    /// the root app hash
    pub root: KdHash,

    /// the root entry hash to hang additional entries from
    pub app_entry_hash: KdHash,

    /// the agent pubkey
    pub agent: KdHash,

    /// the kdirect instance handle
    pub kdirect: KitsuneDirect,

    /// the control handle to the node instance
    pub kdhnd: KdHnd,
}

/// callback type for agent hook execution
pub type AgentHook = Box<dyn FnMut(AgentHookInput) -> AgentHookResp + 'static + Send>;

/// configuration for spawning KdTestHarness
pub struct KdTestConfig {
    /// which kdirect ver to run
    pub ver: KdVerSpec,

    /// how many nodes to create
    pub node_count: usize,

    /// how hany agents to join on each node
    pub agents_per_node: usize,

    /// logic to be invoked on each agent on init
    pub agent_init_hook: AgentHook,

    /// how often to call the periodic agent hook (None for never)
    pub periodic_agent_hook_interval_ms: Option<u64>,

    /// logic to be invoked on the periodic agent hook interval
    pub periodic_agent_hook: AgentHook,
}

impl Default for KdTestConfig {
    fn default() -> Self {
        Self {
            ver: KdVerSpec::V1,
            node_count: 2,
            agents_per_node: 2,
            agent_init_hook: Box::new(|_| async move { Ok(()) }.boxed()),
            periodic_agent_hook_interval_ms: None,
            periodic_agent_hook: Box::new(|_| async move { Ok(()) }.boxed()),
        }
    }
}

/// handle to an individual test harness node
#[derive(Clone)]
pub struct KdTestNodeHandle {
    /// the agents that were created/joined on this node
    pub local_agents: Vec<KdHash>,

    /// the kdirect node instance
    pub kdirect: KitsuneDirect,

    /// the control handle to the node instance
    pub kdhnd: KdHnd,

    message_box: Share<Vec<KdHndEvt>>,
}

impl KdTestNodeHandle {
    /// collect events emitted by this node
    pub fn collect_events(&self) -> Vec<KdHndEvt> {
        self.message_box
            .share_mut(|i, _| Ok(i.drain(..).collect()))
            .unwrap()
    }
}

/// kdirect test harness
pub struct KdTestHarness {
    /// the root hash
    pub root: KdHash,

    /// the app entry hash to hang additional entries from
    pub app_entry_hash: KdHash,

    /// the list of nodes created for this test run
    pub nodes: Vec<KdTestNodeHandle>,

    proxy_close: CloseCb,
}

impl KdTestHarness {
    /// shut down the test
    pub async fn close(self) {
        let Self {
            nodes, proxy_close, ..
        } = self;

        let mut all = Vec::new();
        for node in nodes.iter() {
            all.push(node.kdirect.close(0, ""));
        }
        futures::future::join_all(all).await;

        proxy_close(0, "").await;

        tracing::info!("DONE");
    }
}

impl KdTestHarness {
    /// spawn a new kdirect test harness
    pub async fn start_test(mut config: KdTestConfig) -> KdResult<Self> {
        let (proxy_url, driver, proxy_close) =
            new_quick_proxy_v1().await.map_err(KdError::other)?;
        metric_task(async move {
            driver.await;
            KdResult::Ok(())
        });

        tracing::info!(%proxy_url);

        let mut nodes = Vec::new();

        let root_persist = new_persist_mem();
        let root = root_persist.generate_signing_keypair().await?;
        tracing::info!(%root);

        let app_entry = KdEntryContent {
            kind: "s.app".to_string(),
            parent: root.clone(),
            author: root.clone(),
            verify: "".to_string(),
            data: serde_json::json!({}),
        };
        let app_entry = KdEntrySigned::from_content(&root_persist, app_entry)
            .await
            .map_err(KdError::other)?;
        tracing::debug!(?app_entry);

        let app_entry_hash = app_entry.hash().clone();

        for _ in 0..config.node_count {
            let persist = new_persist_mem();
            let message_box = Share::new(Vec::new());
            let (kdirect, kdhnd) = match config.ver {
                KdVerSpec::V1 => {
                    let conf = KitsuneDirectV1Config {
                        persist,
                        proxy: proxy_url.clone(),
                        ui_port: 0,
                    };

                    let (kdirect, driver) = new_kitsune_direct_v1(conf).await?;
                    metric_task(async move {
                        driver.await;
                        KdResult::Ok(())
                    });

                    let node_addrs = kdirect.list_transport_bindings().await?;
                    tracing::debug!(?node_addrs);

                    let (kdhnd, mut evt) = kdirect.bind_control_handle().await?;

                    let msg_box = message_box.clone();
                    metric_task(async move {
                        while let Some(evt) = evt.next().await {
                            tracing::trace!(?evt);
                            if msg_box
                                .share_mut(move |i, _| {
                                    i.push(evt);
                                    Ok(())
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        KdResult::Ok(())
                    });

                    (kdirect, kdhnd)
                }
            };

            let mut local_agents = Vec::new();
            for _ in 0..config.agents_per_node {
                let agent = kdirect.get_persist().generate_signing_keypair().await?;
                tracing::info!(%agent);

                kdhnd
                    .app_join(root.clone(), agent.clone())
                    .await
                    .map_err(KdError::other)?;

                // sneak this directly into the db : )
                kdirect
                    .get_persist()
                    .store_entry(root.clone(), agent.clone(), app_entry.clone())
                    .await?;

                let input = AgentHookInput {
                    root: root.clone(),
                    app_entry_hash: app_entry_hash.clone(),
                    agent: agent.clone(),
                    kdirect: kdirect.clone(),
                    kdhnd: kdhnd.clone(),
                };
                (config.agent_init_hook)(input).await?;

                local_agents.push(agent);
            }

            nodes.push(KdTestNodeHandle {
                local_agents,
                kdirect,
                kdhnd,
                message_box,
            });
        }

        if let Some(interval_ms) = config.periodic_agent_hook_interval_ms {
            metric_task(periodic_agent_hook_task(
                interval_ms,
                root.clone(),
                app_entry_hash.clone(),
                nodes.clone(),
                config.periodic_agent_hook,
            ));
        }

        // -- begin bootstrap node info sync -- //
        let mut all_agent_info = Vec::new();
        for node in nodes.iter() {
            for info in node
                .kdirect
                .get_persist()
                .query_agent_info(root.clone())
                .await?
            {
                tracing::debug!(?info);
                all_agent_info.push(info);
            }
        }
        for node in nodes.iter() {
            for info in all_agent_info.iter() {
                node.kdirect
                    .get_persist()
                    .store_agent_info(info.clone())
                    .await?;
            }
        }
        // -- end bootstrap node info sync -- //

        Ok(Self {
            root,
            app_entry_hash: app_entry.hash().clone(),
            nodes,
            proxy_close,
        })
    }
}

async fn periodic_agent_hook_task(
    interval_ms: u64,
    root: KdHash,
    app_entry_hash: KdHash,
    nodes: Vec<KdTestNodeHandle>,
    mut periodic_agent_hook: AgentHook,
) -> KdResult<()> {
    'top: loop {
        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;

        for node in nodes.iter() {
            for agent in node.local_agents.iter() {
                let input = AgentHookInput {
                    root: root.clone(),
                    app_entry_hash: app_entry_hash.clone(),
                    agent: agent.clone(),
                    kdirect: node.kdirect.clone(),
                    kdhnd: node.kdhnd.clone(),
                };
                if periodic_agent_hook(input).await.is_err() {
                    break 'top;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn sanity_run_for_five_seconds() {
        init_tracing();

        let mut config = KdTestConfig::default();
        config.node_count = 2;
        config.agents_per_node = 2;
        config.agent_init_hook = Box::new(|input| {
            async move {
                let AgentHookInput {
                    root,
                    app_entry_hash,
                    agent,
                    kdirect: _,
                    kdhnd,
                } = input;

                let new_entry = KdEntryContent {
                    kind: "u.foo".to_string(),
                    parent: app_entry_hash,
                    author: agent.clone(),
                    verify: "".to_string(),
                    data: serde_json::json!({
                        "nonce": std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs_f64(),
                    }),
                };
                let new_entry = kdhnd
                    .entry_author(
                        root.clone(),
                        agent.clone(),
                        new_entry,
                        vec![].into_boxed_slice().into(),
                    )
                    .await
                    .map_err(KdError::other)?;
                tracing::debug!(?new_entry);

                Ok(())
            }
            .boxed()
        });

        let test = KdTestHarness::start_test(config).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        assert_eq!(2, test.nodes.len());
        for node in test.nodes.iter() {
            assert_eq!(2, node.local_agents.len());
            for agent in node.local_agents.iter() {
                let entries = node
                    .kdirect
                    .get_persist()
                    .query_entries(
                        test.root.clone(),
                        agent.clone(),
                        f32::MIN,
                        f32::MAX,
                        DhtArc::new(0, u32::MAX),
                    )
                    .await
                    .unwrap();
                let entry_count = entries.len();
                let entry_hashes = entries.iter().map(|e| e.hash()).collect::<Vec<_>>();
                tracing::info!(%entry_count, ?entry_hashes);

                // each of 4 nodes publish 1 entry + the app entry == 5
                assert_eq!(5, entry_count);
            }
        }

        test.close().await;
    }
}
