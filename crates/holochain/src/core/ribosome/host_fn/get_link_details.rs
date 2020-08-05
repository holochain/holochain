use crate::core::ribosome::error::{RibosomeError, RibosomeResult};
use crate::core::{
    ribosome::{CallContext, RibosomeT},
    state::{cascade::error::CascadeResult, metadata::LinkMetaKey},
    workflow::CallZomeWorkspace,
};
use futures::future::FutureExt;
use holochain_p2p::actor::GetLinksOptions;
use holochain_zome_types::link::Link;
use holochain_zome_types::GetLinksInput;
use holochain_zome_types::GetLinksOutput;
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
        |workspace: &'a mut CallZomeWorkspace| -> MustBoxFuture<'a, CascadeResult<Vec<Link>>> {
            async move {
                let mut cascade = workspace.cascade(network);

                // Create the key
                let key = match tag.as_ref() {
                    Some(tag) => LinkMetaKey::BaseZomeTag(&base_address, zome_id, tag),
                    None => LinkMetaKey::BaseZome(&base_address, zome_id),
                };

                // Get the links from the dht
                cascade
                    .dht_get_link_details(&key, GetLinksOptions::default())
                    .await
            }
            .boxed()
            .into()
        };

    let links = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        unsafe { call_context.host_access.workspace().apply_mut(call).await }
    })??;

    Ok(GetLinkDetailsOutput::new(links.into()))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod slow_tests {
    use crate::core::state::workspace::Workspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use hdk3::prelude::*;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
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

        let foo_bar: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            TestString::from("foo.bar".to_string())
        );

        let foo_baz: holo_hash::EntryHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "hash",
            TestString::from("foo.baz".to_string())
        );

        let children_output: holochain_zome_types::link::LinkDetails = crate::call_test_ribosome!(
            host_access,
            TestWasm::HashPath,
            "children_details",
            TestString::from("foo".to_string())
        );

        let links = children_output.into_inner();
        assert_eq!(2, links.len());
        assert_eq!(links[0].target, foo_baz,);
        assert_eq!(links[1].target, foo_bar,);
    }
}
