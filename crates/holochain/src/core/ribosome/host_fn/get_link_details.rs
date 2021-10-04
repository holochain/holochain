use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetLinksOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use futures::future::join_all;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_link_details<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetLinksInput>,
) -> Result<Vec<LinkDetails>, WasmError> {
    match HostFnAccess::from(call_context.host_context()) {
        HostFnAccess{ read_workspace: Permission::Allow, .. } => {
            let results: Vec<Result<Vec<_>, _>> = tokio_helper::block_forever_on(async move {
                join_all(inputs.into_iter().map(|input| {
                    async {
                        let GetLinksInput {
                            base_address,
                            tag_prefix,
                        } = input;
                        let zome_id = ribosome
                            .zome_to_id(&call_context.zome)
                            .expect("Failed to get ID for current zome.");
                        let key = WireLinkKey {
                            base: base_address,
                            zome_id,
                            tag: tag_prefix,
                        };
                        Cascade::from_workspace_network(
                            call_context.host_context.workspace(),
                            call_context.host_context.network().to_owned(),
                        ).get_link_details(key, GetLinksOptions::default()).await
                    }
                })).await
            });
            let results: Result<Vec<_>, _> = results.into_iter().map(|result|
                match result {
                    Ok(v) => Ok(v.into()),
                    Err(cascade_error) => Err(WasmError::Host(cascade_error.to_string())),
                }
            ).collect();
            Ok(results?)
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::element::SignedHeaderHashed;
    use holochain_zome_types::Header;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_entry_hash_path_children_details() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        // ensure foo.bar twice to ensure idempotency
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.bar".to_string()
        ).unwrap();
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.bar".to_string()
        ).unwrap();

        // ensure foo.baz
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.baz".to_string()
        ).unwrap();

        let exists_output: bool = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "exists",
            "foo".to_string()
        ).unwrap();

        assert_eq!(true, exists_output,);

        let _foo_bar: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            "foo.bar".to_string()
        ).unwrap();

        let _foo_baz: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            "foo.baz".to_string()
        ).unwrap();

        let children_details_output: holochain_zome_types::link::LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children_details",
            "foo".to_string()
        ).unwrap();

        let link_details = children_details_output.into_inner();

        let to_remove: SignedHeaderHashed = (link_details[0]).0.clone();

        let to_remove_hash = to_remove.as_hash().clone();

        let _remove_hash: holo_hash::HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "delete_link",
            to_remove_hash
        ).unwrap();

        let children_details_output_2: holochain_zome_types::link::LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children_details",
            "foo".to_string()
        ).unwrap();

        let children_details_output_2_vec = children_details_output_2.into_inner();
        assert_eq!(2, children_details_output_2_vec.len());

        let mut remove_happened = false;
        for (_, removes) in children_details_output_2_vec {
            if removes.len() > 0 {
                remove_happened = true;

                let link_add_address = unwrap_to
                    ::unwrap_to!(removes[0].header() => Header::DeleteLink)
                .link_add_address
                .clone();
                assert_eq!(link_add_address, to_remove_hash,);
            }
        }
        assert!(remove_happened);
    }
}
