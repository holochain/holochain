use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_details<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetInput,
) -> Result<Option<Details>, WasmError> {
    let GetInput{ any_dht_hash, get_options } = input;

    // Get the network from the context
    let network = call_context.host_access.network().clone();

    // timeouts must be handled by the network
    tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let maybe_details = call_context
            .host_access
            .workspace()
            .write()
            .await
            .cascade(network)
            .get_details(any_dht_hash, get_options)
            .await
            .map_err(|cascade_error| WasmError::Host(cascade_error.to_string()))?;
        Ok(maybe_details)
    })
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use hdk::prelude::*;
    use crate::core::workflow::CallZomeWorkspace;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use holochain_wasm_test_utils::TestWasm;
    use ::fixt::prelude::*;

    #[tokio::test(threaded_scheduler)]
    async fn ribosome_get_details_test<'a>() {
        observability::test_run().ok();

        let test_env = holochain_sqlite::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock =
            crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock.clone();

        // simple replica of the internal type for the TestWasm::Crud entry
        #[derive(Clone, Copy, Serialize, Deserialize, SerializedBytes, Debug, PartialEq)]
        struct CounTree(u32);

        let check = |details: Option<Details>, count, delete| match details {
            Some(Details::Element(ref element_details)) => {
                match element_details.element.entry().to_app_option::<CounTree>() {
                    Ok(Some(CounTree(u))) => assert_eq!(u, count),
                    _ => panic!("failed to deserialize {:?}, {}, {}", details, count, delete),
                }
                assert_eq!(element_details.deletes.len(), delete);
            }
            _ => panic!("no element"),
        };

        let check_entry = |details: Option<Details>, count, update, delete, line| match details
        {
            Some(Details::Entry(ref entry_details)) => {
                match entry_details.entry {
                    Entry::App(ref eb) => {
                        let countree = CounTree::try_from(eb.clone().into_sb()).unwrap();
                        assert_eq!(countree, CounTree(count));
                    }
                    _ => panic!(
                        "failed to deserialize {:?}, {}, {}, {}",
                        details, count, update, delete
                    ),
                }
                assert_eq!(entry_details.updates.len(), update, "{}", line);
                assert_eq!(entry_details.deletes.len(), delete, "{}", line);
            }
            _ => panic!("no entry"),
        };

        let zero_hash: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_hash", CounTree(0));
        let one_hash: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_hash", CounTree(1));
        let two_hash: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_hash", CounTree(2));

        let zero_a: HeaderHash = crate::call_test_ribosome!(host_access, TestWasm::Crud, "new", ());
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", zero_a),
            0,
            0,
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", zero_hash),
            0,
            0,
            0,
            line!(),
        );

        let one_a: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", zero_a);
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", zero_a),
            0,
            0,
        );
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", one_a),
            1,
            0,
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", zero_hash),
            0,
            1,
            0,
            line!(),
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", one_hash),
            1,
            0,
            0,
            line!(),
        );

        let one_b: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", zero_a);
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", zero_a),
            0,
            0,
        );
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", one_b),
            1,
            0,
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", zero_hash),
            0,
            2,
            0,
            line!(),
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", one_hash),
            1,
            0,
            0,
            line!(),
        );

        let two: HeaderHash = crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", one_b);
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", one_b),
            1,
            0,
        );
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", two),
            2,
            0,
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", zero_hash),
            0,
            2,
            0,
            line!(),
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", one_hash),
            1,
            1,
            0,
            line!(),
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", two_hash),
            2,
            0,
            0,
            line!(),
        );

        let zero_b: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "dec", one_a);
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", one_a),
            1,
            1,
        );
        check(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", one_b),
            1,
            0,
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", zero_hash),
            0,
            2,
            0,
            line!(),
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", one_hash),
            1,
            1,
            1,
            line!(),
        );
        check_entry(
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_details", two_hash),
            2,
            0,
            0,
            line!(),
        );

        let zero_b_details: Option<Details> =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "header_details", zero_b);
        match zero_b_details {
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
