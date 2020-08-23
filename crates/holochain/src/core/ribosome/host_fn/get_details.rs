use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{CallContext, RibosomeT};
use crate::core::state::cascade::error::CascadeResult;
use crate::core::workflow::CallZomeWorkspace;
use futures::future::FutureExt;
use holochain_zome_types::{metadata::Details, GetDetailsInput, GetDetailsOutput};
use must_future::MustBoxFuture;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_details<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetDetailsInput,
) -> RibosomeResult<GetDetailsOutput> {
    let (hash, options) = input.into_inner();

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    let call =
        |workspace: &'a mut CallZomeWorkspace| -> MustBoxFuture<'a, CascadeResult<Option<Details>>> {
            async move {
                let mut cascade = workspace.cascade(network);
                Ok(cascade.get_details(hash, options.into()).await?)
            }
            .boxed()
            .into()
        };
    // timeouts must be handled by the network
    let maybe_details: Option<Details> =
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            unsafe { call_context.host_access.workspace().apply_mut(call).await }
        })??;
    Ok(GetDetailsOutput::new(maybe_details))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use hdk3::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_get_details_test<'a>() {
        holochain_types::observability::test_run().ok();

        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
        let mut workspace = <crate::core::workflow::call_zome_workflow::CallZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace.clone();

        // simple replica of the internal type for the TestWasm::Crud entry
        #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
        struct CounTree(u32);

        let check = |details: GetDetailsOutput, count, delete| match details.clone().into_inner() {
            Some(Details::Element(element_details)) => {
                match element_details.element.entry().to_app_option::<CounTree>() {
                    Ok(Some(CounTree(u))) => assert_eq!(u, count),
                    _ => panic!("failed to deserialize {:?}, {}, {}", details, count, delete),
                }
                assert_eq!(element_details.deletes.len(), delete);
            }
            _ => panic!("no element"),
        };

        let zero_a: HeaderHash = crate::call_test_ribosome!(host_access, TestWasm::Crud, "new", ());
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", zero_a),
            0,
            0,
        );

        let one_a: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", zero_a);
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", zero_a),
            0,
            0,
        );
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", one_a),
            1,
            0,
        );

        let one_b: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", zero_a);
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", zero_a),
            0,
            0,
        );
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", one_b),
            1,
            0,
        );

        let two: HeaderHash = crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", one_b);
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", one_b),
            1,
            0,
        );
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", two),
            2,
            0,
        );

        let zero_b: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "dec", one_a);
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", one_a),
            1,
            1,
        );
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", one_b),
            1,
            0,
        );

        let zero_b_details: GetDetailsOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "details", zero_b);
        match zero_b_details.into_inner() {
            Some(Details::Element(element_details)) => {
                match element_details.element.entry().as_option() {
                    None => {
                        // this is the delete so it should be none
                    }
                    _ => panic!("delete had an element"),
                }
            }
            _ => panic!("no element"),
        }
    }
}
