use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use futures::future::join_all;
use holochain_cascade::CascadeImpl;
use holochain_p2p::actor::GetLinksOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_link_details<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetLinksInput>,
) -> Result<Vec<LinkDetails>, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            let results: Vec<Result<Vec<_>, RibosomeError>> =
                tokio_helper::block_forever_on(async move {
                    join_all(inputs.into_iter().map(|input| async {
                        let GetLinksInput {
                            base_address,
                            get_options,
                            link_type,
                            tag_prefix,
                            ..
                        } = input;

                        let key = WireLinkKey {
                            base: base_address,
                            type_query: link_type,
                            tag: tag_prefix,
                            after: None,
                            before: None,
                            author: None,
                        };
                        Ok(CascadeImpl::from_workspace_and_network(
                            &call_context.host_context.workspace(),
                            call_context.host_context.network().to_owned(),
                        )
                        .get_link_details(
                            key,
                            GetLinksOptions {
                                get_options,
                                ..Default::default()
                            },
                        )
                        .await?)
                    }))
                    .await
                });
            let results: Result<Vec<_>, RuntimeError> = results
                .into_iter()
                .map(|result| match result {
                    Ok(v) => Ok(v.into()),
                    Err(cascade_error) => {
                        Err(wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into())
                    }
                })
                .collect();
            Ok(results?)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get_link_details".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::sweettest::{SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
    use holo_hash::ActionHash;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::link::LinkDetails;
    use holochain_zome_types::record::SignedActionHashed;
    use holochain_zome_types::Action;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_entry_hash_path_children_details() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::HashPath).await;

        // ensure foo.bar twice to ensure idempotency
        let _: () = conductor
            .call(&alice, "ensure", "foo.bar".to_string())
            .await;

        let _: () = conductor
            .call(&alice, "ensure", "foo.bar".to_string())
            .await;

        // ensure foo.baz
        let _: () = conductor
            .call(&alice, "ensure", "foo.baz".to_string())
            .await;

        let exists_output: bool = conductor.call(&alice, "exists", "foo".to_string()).await;

        assert_eq!(true, exists_output,);

        let _foo_bar: holo_hash::EntryHash = conductor
            .call(&alice, "path_entry_hash", "foo.bar".to_string())
            .await;

        let _foo_baz: holo_hash::EntryHash = conductor
            .call(&alice, "path_entry_hash", "foo.baz".to_string())
            .await;

        let children_details_output: holochain_zome_types::link::LinkDetails = conductor
            .call(&alice, "children_details", "foo".to_string())
            .await;

        let link_details = children_details_output.into_inner();

        let to_remove: SignedActionHashed = (link_details[0]).0.clone();

        let to_remove_hash = to_remove.as_hash().clone();

        let _remove_hash: holo_hash::ActionHash = conductor
            .call(&alice, "delete_link", to_remove_hash.clone())
            .await;

        let children_details_output_2: holochain_zome_types::link::LinkDetails = conductor
            .call(&alice, "children_details", "foo".to_string())
            .await;

        let children_details_output_2_vec = children_details_output_2.into_inner();
        assert_eq!(2, children_details_output_2_vec.len());

        let mut remove_happened = false;
        for (_, removes) in children_details_output_2_vec {
            if removes.len() > 0 {
                remove_happened = true;

                let link_add_address = unwrap_to
                    ::unwrap_to!(removes[0].action() => Action::DeleteLink)
                .link_add_address
                .clone();
                assert_eq!(link_add_address, to_remove_hash,);
            }
        }
        assert!(remove_happened);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_link_details_local_only() {
        holochain_trace::test_run();
        // agents should not pass around data
        let config = SweetConductorConfig::rendezvous(false).tune(|config| {
            config.disable_historical_gossip = true;
            config.disable_recent_gossip = true;
            config.disable_publish = true;
        });
        let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;
        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Link]).await;
        let apps = conductors.setup_app("test", &[dna_file]).await.unwrap();

        // alice creates a link
        let zome_alice = apps[0].cells()[0].zome(TestWasm::Link.coordinator_zome_name());
        let _: ActionHash = conductors[0].call(&zome_alice, "create_link", ()).await;

        // now make both agents aware of each other
        conductors.exchange_peer_info().await;

        // bob gets link details locally only
        let zome_bob = apps[1].cells()[0].zome(TestWasm::Link.coordinator_zome_name());
        let local_link_details: LinkDetails = conductors[1]
            .call(&zome_bob, "get_link_details_local_only", ())
            .await;
        // link details should be empty
        assert_eq!(local_link_details.into_inner().len(), 0);
    }
}
