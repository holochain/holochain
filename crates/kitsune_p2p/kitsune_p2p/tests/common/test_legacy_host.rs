use futures::{channel::mpsc::Receiver, FutureExt, StreamExt};
use kitsune_p2p::event::KitsuneP2pEvent;
use kitsune_p2p_bin_data::{KitsuneAgent, KitsuneSignature};
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    dht::{
        arq::LocalStorageConfig,
        spacetime::{Dimension, Topology},
        ArqStrat, PeerStrat,
    },
};
use std::{collections::HashSet, sync::Arc};

use super::test_keystore;

pub struct TestLegacyHost {
    _handle: tokio::task::JoinHandle<()>,
    keystore: Arc<
        futures::lock::Mutex<
            kitsune_p2p_types::dependencies::lair_keystore_api::prelude::LairClient,
        >,
    >,
}

impl TestLegacyHost {
    pub async fn start(agent_store: Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>>, receivers: Vec<Receiver<KitsuneP2pEvent>>) -> Self {
        let keystore = test_keystore();

        let handle = tokio::task::spawn({
            let keystore = keystore.clone();
            async move {
                let mut receiver = futures::stream::select_all(receivers).fuse();
                while let Some(evt) = receiver.next().await {
                    match evt {
                        KitsuneP2pEvent::PutAgentInfoSigned { respond, input, .. } => {
                            let mut store = agent_store.write();
                            let incoming_agents: HashSet<_> =
                                input.peer_data.iter().map(|p| p.agent.clone()).collect();
                            store.retain(|p: &AgentInfoSigned| !incoming_agents.contains(&p.agent));
                            store.extend(input.peer_data);
                            respond.respond(Ok(async move { Ok(()) }.boxed().into()))
                        }
                        KitsuneP2pEvent::QueryAgents { respond, input, .. } => {
                            let store = agent_store.read();
                            let agents = store
                                .iter()
                                .filter(|p| p.space == input.space)
                                .cloned()
                                .collect::<Vec<_>>();
                            respond.respond(Ok(async move { Ok(agents) }.boxed().into()))
                        }
                        KitsuneP2pEvent::QueryPeerDensity {
                            respond,
                            space,
                            dht_arc,
                            ..
                        } => {
                            let cutoff = std::time::Duration::from_secs(60 * 15);
                            let topology = Topology {
                                space: Dimension::standard_space(),
                                time: Dimension::time(std::time::Duration::from_secs(60 * 5)),
                                time_origin: Timestamp::now(),
                                time_cutoff: cutoff,
                            };
                            let now = Timestamp::now().as_millis() as u64;
                            let arcs = agent_store
                                .read()
                                .iter()
                                .filter_map(|agent: &AgentInfoSigned| {
                                    if agent.space == space && now < agent.expires_at_ms {
                                        Some(agent.storage_arc.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>();

                            let strat = PeerStrat::Quantized(ArqStrat::standard(
                                LocalStorageConfig::default(),
                            ));
                            let view = strat.view(topology, dht_arc, &arcs);

                            respond.respond(Ok(async move { Ok(view) }.boxed().into()))
                        }
                        KitsuneP2pEvent::Call { respond, payload, .. } => {
                            // Echo the request payload
                            respond.respond(Ok(async move { Ok(payload) }.boxed().into()))
                        }
                        KitsuneP2pEvent::QueryOpHashes { respond, .. } => {
                            // TODO nothing to send yet
                            respond.respond(Ok(async move { Ok(None) }.boxed().into()))
                        }
                        KitsuneP2pEvent::SignNetworkData { respond, input, .. } => {
                            let mut key = [0; 32];
                            key.copy_from_slice(&input.agent.0.as_slice());
                            let sig = keystore
                                .lock()
                                .await
                                .sign_by_pub_key(
                                    key.into(),
                                    None,
                                    input.data.as_slice().to_vec().into(),
                                )
                                .await
                                .unwrap();
                            respond.respond(Ok(async move { Ok(KitsuneSignature(sig.0.to_vec())) }
                                .boxed()
                                .into()))
                        }
                        _ => todo!("Unhandled event {:?}", evt),
                    }
                }
            }
        });

        Self {
            _handle: handle,
            keystore,
        }
    }

    pub async fn create_agent(&self) -> KitsuneAgent {
        let ks = self.keystore.lock().await;
        let tag = nanoid::nanoid!();
        let info = ks.new_seed(tag.into(), None, false).await.unwrap();
        KitsuneAgent(info.ed25519_pub_key.0.to_vec())
    }
}
