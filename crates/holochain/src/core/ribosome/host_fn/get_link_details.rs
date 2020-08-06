use crate::core::ribosome::error::{RibosomeError, RibosomeResult};
use crate::core::{
    ribosome::{CallContext, RibosomeT},
    state::{cascade::error::CascadeResult, metadata::LinkMetaKey},
    workflow::CallZomeWorkspace,
};
use futures::future::FutureExt;
use holochain_p2p::actor::GetLinksOptions;
use holochain_zome_types::link::LinkDetails;
use holochain_zome_types::GetLinkDetailsInput;
use holochain_zome_types::GetLinkDetailsOutput;
use must_future::MustBoxFuture;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_link_details<'a>(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetLinkDetailsInput,
) -> RibosomeResult<GetLinkDetailsOutput> {
    let (base_address, tag) = input.into_inner();

    // Get zome id
    let zome_id: holochain_zome_types::header::ZomeId = match ribosome
        .dna_file()
        .dna
        .zomes
        .iter()
        .position(|(name, _)| name == &call_context.zome_name)
    {
        Some(index) => holochain_zome_types::header::ZomeId::from(index as u8),
        None => Err(RibosomeError::ZomeNotExists(call_context.zome_name.clone()))?,
    };

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    let call =
        |workspace: &'a mut CallZomeWorkspace| -> MustBoxFuture<'a, CascadeResult<LinkDetails>> {
            async move {
                let mut cascade = workspace.cascade(network);

                // Create the key
                let key = match tag.as_ref() {
                    Some(tag) => LinkMetaKey::BaseZomeTag(&base_address, zome_id, tag),
                    None => LinkMetaKey::BaseZome(&base_address, zome_id),
                };

                // Get the links from the dht
                Ok(LinkDetails::from(
                    cascade
                        .get_link_details(&key, GetLinksOptions::default())
                        .await?,
                ))
            }
            .boxed()
            .into()
        };

    let link_details = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        unsafe { call_context.host_access.workspace().apply_mut(call).await }
    })??;

    Ok(GetLinkDetailsOutput::new(link_details))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::core::state::workspace::Workspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holo_hash::HasHash;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::header::LinkAdd;
    use test_wasm_common::*;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_entry_hash_path_children_details() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;

        let reader = env_ref.reader().unwrap();
        let mut workspace = crate::core::workflow::CallZomeWorkspace::new(&reader, &dbs).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        // ensure foo.bar twice to ensure idempotency
        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;

        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            TestString::from("foo.bar".to_string())
        );
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            TestString::from("foo.bar".to_string())
        );

        // ensure foo.baz
        let _: () = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "ensure",
            TestString::from("foo.baz".to_string())
        );

        let exists_output: TestBool = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "exists",
            TestString::from("foo".to_string())
        );

        assert_eq!(TestBool(true), exists_output,);

        let _foo_bar: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            TestString::from("foo.bar".to_string())
        );

        let _foo_baz: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            TestString::from("foo.baz".to_string())
        );

        let children_details_output: holochain_zome_types::link::LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children_details",
            TestString::from("foo".to_string())
        );

        let link_details = children_details_output.into_inner();

        let to_remove: LinkAdd = (link_details[0]).0.clone();

        let to_remove_hash = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            holochain_types::header::HeaderHashed::from_content(to_remove.into()).await
        })
        .into_hash();

        let _remove_hash: holo_hash::HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "remove_link",
            to_remove_hash
        );

        let children_details_output_2: holochain_zome_types::link::LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children_details",
            TestString::from("foo".to_string())
        );

        let children_details_output_2_vec = children_details_output_2.into_inner();
        assert_eq!(2, children_details_output_2_vec.len());

        let mut remove_happened = false;
        for (_, removes) in children_details_output_2_vec {
            if removes.len() > 0 {
                remove_happened = true;

                assert_eq!(&removes[0].link_add_address, &to_remove_hash,);
            }
        }
        assert!(remove_happened);
    }
}
