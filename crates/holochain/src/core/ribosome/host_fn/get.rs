use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use futures::StreamExt;
use holochain_cascade::CascadeImpl;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[cfg_attr(feature = "instrument", tracing::instrument(skip(_ribosome, call_context), fields(?call_context.zome, function = ?call_context.function_name)))]
pub fn get(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetInput>,
) -> Result<Vec<Option<Record>>, RuntimeError> {
    let num_requests = inputs.len();
    tracing::debug!("Starting with {} requests.", num_requests);
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            let results: Vec<Result<Option<Record>, _>> =
                tokio_helper::block_forever_on(async move {
                    futures::stream::iter(inputs.into_iter().map(|input| async {
                        let GetInput {
                            any_dht_hash,
                            get_options,
                        } = input;
                        CascadeImpl::from_workspace_and_network(
                            &call_context.host_context.workspace(),
                            call_context.host_context.network().clone(),
                        )
                        .dht_get(any_dht_hash, get_options)
                        .await
                    }))
                    // Limit concurrent calls to 10 as each call
                    // can spawn multiple connections.
                    .buffered(10)
                    .collect()
                    .await
                });
            let results: Result<Vec<_>, RuntimeError> = results
                .into_iter()
                .map(|result| match result {
                    Ok(v) => Ok(v),
                    Err(cascade_error) => {
                        Err(wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into())
                    }
                })
                .collect();
            let results = results?;
            tracing::debug!(
                "Ending with {} out of {} results and {} total responses.",
                results.iter().filter(|r| r.is_some()).count(),
                num_requests,
                results.len(),
            );
            Ok(results)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

// we are relying on the create tests to show the commit/get round trip
// See create.rs

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::sweettest::{SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
    use holo_hash::ActionHash;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::record::Record;

    #[tokio::test(flavor = "multi_thread")]
    async fn get_action_entry_local_only() {
        holochain_trace::test_run();
        // agents should not pass around data
        let config = SweetConductorConfig::rendezvous(false)
            .tune(|config| {
                config.disable_historical_gossip = true;
                config.disable_recent_gossip = true;
                config.disable_publish = true;
            })
            .no_dpki();
        let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;
        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let apps = conductors.setup_app("test", &[dna_file]).await.unwrap();

        // alice creates an entry
        let zome_alice = apps[0].cells()[0].zome(TestWasm::Create.coordinator_zome_name());
        let entry_action_hash: ActionHash =
            conductors[0].call(&zome_alice, "create_entry", ()).await;
        let local_record_by_action_hash: Option<Record> = conductors[0]
            .call(&zome_alice, "get_post", entry_action_hash.clone())
            .await;
        // alice can get the record
        assert!(local_record_by_action_hash.is_some());

        // now make both agents aware of each other
        conductors.exchange_peer_info().await;

        // bob gets record by action hash from local databases
        let zome_bob = apps[1].cells()[0].zome(TestWasm::Create.coordinator_zome_name());
        let local_record_by_action_hash: Option<Record> = conductors[1]
            .call(&zome_bob, "get_post", entry_action_hash)
            .await;
        // record should be none
        assert!(local_record_by_action_hash.is_none());

        // bob gets record by entry hash from local databases
        let zome_bob = apps[1].cells()[0].zome(TestWasm::Create.coordinator_zome_name());
        let local_record_by_entry_hash: Option<Record> =
            conductors[1].call(&zome_bob, "get_entry", ()).await;
        // record should be none
        assert!(local_record_by_entry_hash.is_none());
    }
}
