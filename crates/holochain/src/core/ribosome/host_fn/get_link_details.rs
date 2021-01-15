use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_state::metadata::LinkMetaKey;
use holochain_p2p::actor::GetLinksOptions;
use holochain_types::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_link_details<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetLinksInputInner,
) -> RibosomeResult<LinkDetails> {
    let GetLinksInputInner { base_address, tag_prefix } = input;

    // Get zome id
    let zome_id = ribosome.zome_to_id(&call_context.zome)?;

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        // Create the key
        let key = match tag_prefix.as_ref() {
            Some(tag_prefix) => LinkMetaKey::BaseZomeTag(&base_address, zome_id, tag_prefix),
            None => LinkMetaKey::BaseZome(&base_address, zome_id),
        };

        // Get the links from the dht
        let link_details = LinkDetails::from(
            call_context
                .host_access
                .workspace()
                .write()
                .await
                .cascade(network)
                .get_link_details(&key, GetLinksOptions::default())
                .await?,
        );

        Ok(link_details)
    })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::element::SignedHeaderHashed;
    use holochain_zome_types::Header;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_entry_hash_path_children_details() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();

        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;

        // ensure foo.bar twice to ensure idempotency
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.bar".to_string()
        );
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.bar".to_string()
        );

        // ensure foo.baz
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            "foo.baz".to_string()
        );

        let exists_output: bool = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "exists",
            "foo".to_string()
        );

        assert_eq!(true, exists_output,);

        let _foo_bar: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            "foo.bar".to_string()
        );

        let _foo_baz: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            "foo.baz".to_string()
        );

        let children_details_output: holochain_zome_types::link::LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children_details",
            "foo".to_string()
        );

        let link_details = children_details_output.into_inner();

        let to_remove: SignedHeaderHashed = (link_details[0]).0.clone();

        let to_remove_hash = to_remove.as_hash().clone();

        let _remove_hash: holo_hash::HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "delete_link",
            to_remove_hash
        );

        let children_details_output_2: holochain_zome_types::link::LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children_details",
            "foo".to_string()
        );

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
