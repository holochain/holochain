use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_p2p::HolochainP2pCellT;
use holochain_serialized_bytes::prelude::SerializedBytes;
use holochain_zome_types::signal::RemoteSignal;
use holochain_zome_types::zome::FunctionName;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::RemoteSignalInput;
use holochain_zome_types::RemoteSignalOutput;
use std::convert::TryInto;
use std::sync::Arc;

pub fn remote_signal(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: RemoteSignalInput,
) -> RibosomeResult<RemoteSignalOutput> {
    const FN_NAME: &'static str = "remote_signal";
    // it is the network's responsibility to handle timeouts and return an Err result in that case
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let network = call_context.host_access().network().clone();
        let RemoteSignal { agents, signal } = input.into_inner();
        let zome_name: ZomeName = call_context.zome().into();
        let fn_name: FunctionName = FN_NAME.into();
        let request: SerializedBytes = signal.try_into()?;
        for agent in agents {
            tokio::task::spawn({
                let mut network = network.clone();
                let zome_name = zome_name.clone();
                let fn_name = fn_name.clone();
                let request = request.clone();
                async move {
                    let result = network
                        .call_remote(agent.clone(), zome_name, fn_name, None, request)
                        .await;
                    if let Err(e) = result {
                        tracing::info!(
                            "Failed to send remote signal to {:?} because of {:?}",
                            agent,
                            e
                        );
                    }
                }
            });
        }
        Ok(())
    })
    .map(RemoteSignalOutput::new)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::conductor::{config::ConductorConfig, p2p_store::exchange_peer_info, Conductor};
    use crate::test_utils::test_conductor::{TestAgents, TestConductorHandle};
    use futures::future;
    use hdk3::prelude::*;
    use holochain_state::test_utils::test_environments;
    use holochain_types::dna::{zome::inline_zome::InlineZome, DnaFile};
    use holochain_zome_types::signal::AppSignal;
    use kitsune_p2p::KitsuneP2pConfig;

    #[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
    #[serde(transparent)]
    #[repr(transparent)]
    struct AppString(String);

    fn zome(agents: Vec<AgentPubKey>, num_signals: Arc<AtomicUsize>) -> InlineZome {
        let entry_def = EntryDef::default_with_id("entrydef");

        InlineZome::new_unique(vec![entry_def.clone()])
            .callback("signal_others", move |api, ()| {
                let signal = AppSignal::new(AppString("Hey".to_string()).try_into().unwrap());
                let remote_signal = RemoteSignal {
                    agents: agents.clone(),
                    signal,
                };
                tracing::debug!("sending signal to {:?}", agents);
                api.remote_signal(remote_signal)?;
                Ok(())
            })
            .callback("remote_signal", move |api, signal: AppSignal| {
                tracing::debug!("remote signal");
                num_signals.fetch_add(1, Ordering::SeqCst);
                api.emit_signal(signal).map_err(Into::into)
            })
            .callback("init", move |api, ()| {
                let mut functions: GrantedFunctions = HashSet::new();
                functions.insert((api.zome_info(()).unwrap().zome_name, "remote_signal".into()));
                let cap_grant_entry = CapGrantEntry {
                    tag: "".into(),
                    // empty access converts to unrestricted
                    access: ().into(),
                    functions,
                };
                api.create((EntryDefId::CapGrant, Entry::CapGrant(cap_grant_entry)))
                    .unwrap();

                Ok(InitCallbackResult::Pass)
            })
    }

    // TODO [ B-03669 ]: make much less verbose
    #[tokio::test(threaded_scheduler)]
    #[cfg(feature = "test_utils")]
    async fn remote_signal_test() -> anyhow::Result<()> {
        observability::test_run().ok();
        const NUM_CONDUCTORS: usize = 5;
        let mut all_envs = Vec::with_capacity(NUM_CONDUCTORS);
        let mut all_agents = Vec::with_capacity(NUM_CONDUCTORS);
        for _ in 0..NUM_CONDUCTORS {
            let envs = test_environments();
            let agent = TestAgents::one(envs.keystore()).await;
            all_envs.push(envs);
            all_agents.push(agent);
        }
        let index = AtomicUsize::new(0);
        let index_ref = &index;
        let envs_ref = &all_envs;
        let num_signals = Arc::new(AtomicUsize::new(0));

        let conductors = future::join_all(
            std::iter::repeat_with(|| async move {
                // let envs = test_environments();
                let i = index_ref.fetch_add(1, Ordering::SeqCst);
                let envs = envs_ref[i].clone();
                let mut network = KitsuneP2pConfig::default();
                network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
                    bind_to: None,
                    override_host: None,
                    override_port: None,
                }];
                let conductor: TestConductorHandle = Conductor::builder()
                    .config(ConductorConfig {
                        network: Some(network),
                        ..Default::default()
                    })
                    .test(&envs)
                    .await
                    .unwrap()
                    .into();
                (conductor, envs)
            })
            .take(NUM_CONDUCTORS),
        )
        .await;

        let (dna_file, _) = DnaFile::unique_from_inline_zome(
            "zome1",
            zome(all_agents.clone(), num_signals.clone()),
        )
        .await
        .unwrap();

        let agents_ref = &all_agents;

        let data = future::join_all(conductors.iter().enumerate().map(|(i, (conductor, envs))| {
            let dna_file = dna_file.clone();
            async move {
                let data = conductor
                    .setup_app_for_agents_with_no_membrane_proof(
                        "app",
                        &[agents_ref[i].clone()],
                        &[dna_file.clone()],
                    )
                    .await;
                (data, envs)
            }
        }))
        .await;

        let p2p_envs = data.iter().map(|(_, envs)| envs.p2p()).collect();
        exchange_peer_info(p2p_envs);

        let _: () = data[0].0[0].1[0].call("zome1", "signal_others", ()).await;

        tokio::time::delay_for(std::time::Duration::from_millis(1000)).await;
        assert_eq!(num_signals.load(Ordering::SeqCst), NUM_CONDUCTORS);

        Ok(())
    }
}
