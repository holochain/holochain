use crate::core::ribosome::error::{RibosomeError, RibosomeResult};
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::state::{
    cascade::error::{CascadeError, CascadeResult},
    source_chain::SourceChainResult,
};
use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_to_cache;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use holo_hash::{EntryHash, HeaderHash};
use holochain_p2p::actor::GetOptions;
use holochain_zome_types::header::builder;
use holochain_zome_types::DeleteEntryInput;
use holochain_zome_types::{element::SignedHeaderHashed, DeleteEntryOutput};
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn delete_entry<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: DeleteEntryInput,
) -> RibosomeResult<DeleteEntryOutput> {
    let removes_address = input.into_inner();

    let removes_entry_address =
        get_original_address(call_context.clone(), removes_address.clone())?;

    let call =
        |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
            async move {
                let source_chain = &mut workspace.source_chain;
                let header_builder = builder::ElementDelete {
                    removes_address,
                    removes_entry_address,
                };
                let header_hash = source_chain.put(header_builder, None).await?;
                let element = source_chain
                    .get_element(&header_hash)
                    .await?
                    .expect("Element we just put in SourceChain must be gettable");
                tracing::debug!(in_delete_entry = ?header_hash);
                integrate_to_cache(
                    &element,
                    workspace.source_chain.elements(),
                    &mut workspace.cache_meta,
                )
                .await
                .map_err(Box::new)?;
                Ok(header_hash)
            }
            .boxed()
        };

    // handle timeouts at the source chain layer
    let header_address =
        tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        }))???;

    Ok(DeleteEntryOutput::new(header_address))
}

#[allow(clippy::extra_unused_lifetimes)]
pub(crate) fn get_original_address<'a>(
    call_context: Arc<CallContext>,
    address: HeaderHash,
) -> RibosomeResult<EntryHash> {
    let network = call_context.host_access.network().clone();
    let original_element_get_call = {
        let address = address.clone();
        |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<
        'a,
        CascadeResult<Option<SignedHeaderHashed>>,
    > {
        async move {
            let mut cascade = workspace.cascade(network);
            // TODO: Think about what options to use here
            cascade
                .get_details(address.clone().into(), GetOptions::default())
                .await?
                .map(|el| {
                    match el {
                        holochain_zome_types::metadata::Details::Element(e) => Ok(e.element.into_inner().0),
                        // Should not be trying to get original headers via EntryHash
                        holochain_zome_types::metadata::Details::Entry(_) => Err(CascadeError::InvalidResponse(address.into())),
                    }
                })
                .transpose()
        }
        .boxed()
    }
    };
    // handle timeouts at the network layer
    let async_call_context = call_context;
    let maybe_original_element: Option<SignedHeaderHashed> =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe {
                async_call_context
                    .host_access
                    .workspace()
                    .apply_mut(original_element_get_call)
                    .await
            }
        })??;

    let entry_address = match maybe_original_element {
        Some(original_element_signed_header_hash) => {
            match original_element_signed_header_hash.header().entry_data() {
                Some((entry_hash, _)) => Ok(entry_hash.clone()),
                _ => Err(RibosomeError::ElementDeps(address.into())),
            }
        }
        None => Err(RibosomeError::ElementDeps(address.into())),
    }?;
    Ok(entry_address)
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::{core::workflow::CallZomeWorkspace, fixt::ZomeCallHostAccessFixturator};
    use fixt::prelude::*;
    use hdk3::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_delete_entry_test<'a>() {
        holochain_types::observability::test_run().ok();

        let test_env = holochain_state::test_utils::test_cell_env();
let env = test_env.env();
        let dbs = env.dbs().await;
        let mut workspace = CallZomeWorkspace::new(env.clone().into(), &dbs)
            .await
            .unwrap();

        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace.clone();

        let thing_a: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crd, "create", ());
        let get_thing: GetOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Crd, "read", thing_a);
        match get_thing.into_inner() {
            Some(element) => assert!(element.entry().as_option().is_some()),

            None => unreachable!(),
        }

        let _: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crd, "delete", thing_a);

        let get_thing: GetOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Crd, "read", thing_a);
        match get_thing.into_inner() {
            None => {
                // this is what we want, deletion => None for a get
            }
            _ => unreachable!(),
        }
    }
}
