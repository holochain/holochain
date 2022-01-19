use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holochain_p2p::HolochainP2pDnaT;
use holochain_types::access::Permission;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::signal::RemoteSignal;
use holochain_zome_types::zome::FunctionName;
use std::sync::Arc;
use tracing::Instrument;
use crate::core::ribosome::RibosomeError;

#[tracing::instrument(skip(_ribosome, call_context, input))]
pub fn remote_signal(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: RemoteSignal,
) -> Result<(), WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_network: Permission::Allow,
            agent_info: Permission::Allow,
            ..
        } => {
            const FN_NAME: &str = "recv_remote_signal";
            let from_agent = super::agent_info::agent_info(_ribosome, call_context.clone(), ())?
                .agent_latest_pubkey;
            // Timeouts and errors are ignored,
            // this is a send and forget operation.
            let network = call_context.host_context().network().clone();
            let RemoteSignal { agents, signal } = input;
            let zome_name = call_context.zome().zome_name().clone();
            let fn_name: FunctionName = FN_NAME.into();
            tokio::task::spawn(
                async move {
                    if let Err(e) = network
                        .remote_signal(from_agent, agents, zome_name, fn_name, None, signal)
                        .await
                    {
                        tracing::info!("Failed to send remote signals because of {:?}", e);
                    }
                }
                .in_current_span(),
            );
            Ok(())
        }
        _ => Err(WasmError::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "remote_signal".into()
        ).to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::sweettest::SweetDnaFile;
    use crate::sweettest::{SweetAgents, SweetConductorBatch};
    use futures::future;
    use hdk::prelude::*;
    use matches::assert_matches;

    fn zome(agents: Vec<AgentPubKey>, num_signals: Arc<AtomicUsize>) -> InlineZome {
        let entry_def = EntryDef::default_with_id("entrydef");

        InlineZome::new_unique(vec![entry_def.clone()])
            .callback("signal_others", move |api, ()| {
                let signal = ExternIO::encode("Hey").unwrap();
                let signal = RemoteSignal {
                    agents: agents.clone(),
                    signal,
                };
                tracing::debug!("sending signal to {:?}", agents);
                api.remote_signal(signal)?;
                Ok(())
            })
            .callback("recv_remote_signal", move |api, signal: ExternIO| {
                tracing::debug!("remote signal");
                num_signals.fetch_add(1, Ordering::SeqCst);
                api.emit_signal(AppSignal::new(signal)).map_err(Into::into)
            })
            .callback("init", move |api, ()| {
                let mut functions: GrantedFunctions = BTreeSet::new();
                functions.insert((api.zome_info(()).unwrap().name, "recv_remote_signal".into()));
                let cap_grant_entry = CapGrantEntry {
                    tag: "".into(),
                    // empty access converts to unrestricted
                    access: ().into(),
                    functions,
                };
                api.create(CreateInput::new(
                    EntryDefId::CapGrant,
                    Entry::CapGrant(cap_grant_entry),
                    ChainTopOrdering::default(),
                ))
                .unwrap();

                Ok(InitCallbackResult::Pass)
            })
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_utils")]
    #[ignore = "Flaky. Gets stuck at end of test"]
    async fn remote_signal_test() -> anyhow::Result<()> {
        observability::test_run().ok();
        const NUM_CONDUCTORS: usize = 5;

        let num_signals = Arc::new(AtomicUsize::new(0));

        let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;
        let agents =
            future::join_all(conductors.iter().map(|c| SweetAgents::one(c.keystore()))).await;

        let (dna_file, _) = SweetDnaFile::unique_from_inline_zome(
            "zome1",
            zome(agents.clone(), num_signals.clone()),
        )
        .await
        .unwrap();

        let apps = conductors
            .setup_app_for_zipped_agents("app", &agents, &[dna_file.clone().into()])
            .await
            .unwrap();

        conductors.exchange_peer_info().await;

        let cells: Vec<_> = apps.cells_flattened();

        let mut signals = Vec::new();
        for h in conductors.iter() {
            signals.push(h.signal_broadcaster().await.subscribe_separately())
        }
        let signals = signals.into_iter().flatten().collect::<Vec<_>>();

        let _: () = conductors[0]
            .call(&cells[0].zome("zome1"), "signal_others", ())
            .await;

        crate::assert_eq_retry_10s!(num_signals.load(Ordering::SeqCst), NUM_CONDUCTORS);

        for mut signal in signals {
            let r = signal.try_recv();
            // Each handle should recv a signal
            assert_matches!(r, Ok(_))
        }

        Ok(())
    }
}
