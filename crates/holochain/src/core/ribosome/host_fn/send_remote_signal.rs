use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::AgentPubKeyExt;
use holochain_nonce::fresh_nonce;
use holochain_p2p::HolochainP2pDnaT;
use holochain_types::access::Permission;
use holochain_types::prelude::AgentPubKey;
use holochain_types::prelude::CellId;
use holochain_types::prelude::Signature;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::prelude::Timestamp;
use holochain_zome_types::signal::RemoteSignal;
use holochain_zome_types::zome::FunctionName;
use holochain_zome_types::zome_io::ZomeCallUnsigned;
use std::sync::Arc;
use tracing::Instrument;
use wasmer::RuntimeError;

#[tracing::instrument(skip(_ribosome, call_context, input))]
pub fn send_remote_signal(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: RemoteSignal,
) -> Result<(), RuntimeError> {
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
                    let mut to_agent_list: Vec<(Signature, AgentPubKey)> = Vec::new();

                    let (nonce, expires_at) = match fresh_nonce(Timestamp::now()) {
                        Ok(nonce) => nonce,
                        Err(e) => {
                            tracing::info!("Failed to get a fresh nonce because of {:?}", e);
                            return;
                        }
                    };

                    for agent in agents {
                        let zome_call_unsigned = ZomeCallUnsigned {
                            provenance: from_agent.clone(),
                            cell_id: CellId::new(network.dna_hash(), agent.clone()),
                            zome_name: zome_name.clone(),
                            fn_name: fn_name.clone(),
                            cap_secret: None,
                            payload: signal.clone(),
                            nonce,
                            expires_at,
                        };
                        let potentially_signature = zome_call_unsigned.provenance.sign_raw(call_context.host_context.keystore(), match zome_call_unsigned.data_to_sign() {
                            Ok(to_sign) => to_sign,
                            Err(e) => {
                                tracing::info!("Failed to serialize zome call for signal because of {:?}", e);
                                return;
                            }
                        }).await;

                        match potentially_signature {
                            Ok(signature) => to_agent_list.push((signature, agent)),
                            Err(e) => {
                                tracing::info!("Failed to sign and send remote signals because of {:?}", e);
                                return;
                            }
                        }
                    }

                    if let Err(e) = network
                        .send_remote_signal(from_agent, to_agent_list, zome_name, fn_name, None, signal, nonce, expires_at)
                        .await
                    {
                        tracing::info!("Failed to send remote signals because of {:?}", e);
                    }
                }
                .in_current_span(),
            );
            Ok(())
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "send_remote_signal".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::sweettest::*;
    use futures::future;
    use hdk::prelude::*;
    use tokio_stream::StreamExt;

    fn test_zome(agents: Vec<AgentPubKey>, num_signals: Arc<AtomicUsize>) -> InlineIntegrityZome {
        let entry_def = EntryDef::default_from_id("entrydef");

        InlineIntegrityZome::new_unique(vec![entry_def.clone()], 0)
            .function("signal_others", move |api, ()| {
                let signal = ExternIO::encode("Hey").unwrap();
                let signal = RemoteSignal {
                    agents: agents.clone(),
                    signal,
                };
                tracing::debug!("sending remote signal to {:?}", agents);
                api.send_remote_signal(signal)?;
                Ok(())
            })
            .function("recv_remote_signal", move |api, signal: ExternIO| {
                tracing::debug!("remote signal");
                num_signals.fetch_add(1, Ordering::SeqCst);
                api.emit_signal(AppSignal::new(signal)).map_err(Into::into)
            })
            .function("init", move |api, ()| {
                let mut fns = BTreeSet::new();
                fns.insert((api.zome_info(()).unwrap().name, "recv_remote_signal".into()));
                let functions = GrantedFunctions::Listed(fns);
                let cap_grant_entry = CapGrantEntry {
                    tag: "".into(),
                    // empty access converts to unrestricted
                    access: ().into(),
                    functions,
                };
                api.create(CreateInput::new(
                    EntryDefLocation::CapGrant,
                    EntryVisibility::Private,
                    Entry::CapGrant(cap_grant_entry),
                    ChainTopOrdering::default(),
                ))
                .unwrap();

                Ok(InitCallbackResult::Pass)
            })
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_utils")]
    async fn remote_signal_test() -> anyhow::Result<()> {
        holochain_trace::test_run().ok();
        const NUM_CONDUCTORS: usize = 5;

        let num_signals = Arc::new(AtomicUsize::new(0));

        let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

        let agents =
            future::join_all(conductors.iter().map(|c| SweetAgents::one(c.keystore()))).await;

        let zome = test_zome(agents.clone(), num_signals.clone());
        let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome", zome)).await;

        let apps = conductors
            .setup_app_for_zipped_agents("app", &agents, &[dna_file.clone()])
            .await
            .unwrap();

        conductors.exchange_peer_info().await;

        let cells: Vec<_> = apps.cells_flattened();

        let mut signals = Vec::new();
        for h in conductors.iter() {
            signals.push(h.signal_broadcaster().subscribe_merged())
        }

        let _: () = conductors[0]
            .call(&cells[0].zome("zome"), "signal_others", ())
            .await;

        crate::assert_eq_retry_10s!(num_signals.load(Ordering::SeqCst), NUM_CONDUCTORS);

        for mut signal in signals {
            signal.next().await.expect("Failed to recv signal");
        }

        Ok(())
    }
}
