use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use futures::future::join_all;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get_details<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetInput>,
) -> Result<Vec<Option<Details>>, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            let results: Vec<Result<Option<Details>, _>> =
                tokio_helper::block_forever_on(async move {
                    join_all(inputs.into_iter().map(|input| async {
                        let GetInput {
                            any_dht_hash,
                            get_options,
                        } = input;
                        Cascade::from_workspace_network(
                            &call_context.host_context.workspace(),
                            call_context.host_context.network().to_owned(),
                        )
                        .get_details(any_dht_hash, get_options)
                        .await
                    }))
                    .await
                });
            let results: Result<Vec<_>, _> = results
                .into_iter()
                .map(|result| {
                    result.map_err(|cascade_error| wasm_error!(WasmErrorInner::Host(cascade_error.to_string())))
                })
                .collect();
            Ok(results?)
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_get_details_test<'a>() {
        observability::test_run().ok();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        // simple replica of the internal type for the TestWasm::Crud entry
        #[derive(Clone, Copy, Serialize, Deserialize, SerializedBytes, Debug, PartialEq)]
        struct CounTree(u32);

        let check = |details: &Option<Details>, count, delete| match details {
            Some(Details::Element(ref element_details)) => {
                match element_details.element.entry().to_app_option::<CounTree>() {
                    Ok(Some(CounTree(u))) => assert_eq!(u, count),
                    _ => panic!("failed to deserialize {:?}, {}, {}", details, count, delete),
                }
                assert_eq!(element_details.deletes.len(), delete);
            }
            _ => panic!("no element"),
        };

        let check_entry = |details: &Option<Details>, count, update, delete, line| match details {
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
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_hash", CounTree(0))
                .unwrap();
        let one_hash: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_hash", CounTree(1))
                .unwrap();
        let two_hash: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "entry_hash", CounTree(2))
                .unwrap();

        let zero_a: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "new", ()).unwrap();
        let header_details_0: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "header_details",
            vec![zero_a.clone()]
        )
        .unwrap();
        let entry_details_0: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "entry_details",
            vec![zero_hash.clone()]
        )
        .unwrap();
        check(&header_details_0[0], 0, 0);
        check_entry(&entry_details_0[0], 0, 0, 0, line!());

        let one_a: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", zero_a).unwrap();
        let header_details_1: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "header_details",
            vec![zero_a.clone(), one_a.clone()]
        )
        .unwrap();
        let entry_details_1: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "entry_details",
            vec![zero_hash.clone(), one_hash.clone()]
        )
        .unwrap();
        check(&header_details_1[0], 0, 0);
        check(&header_details_1[1], 1, 0);
        check_entry(&entry_details_1[0], 0, 1, 0, line!());
        check_entry(&entry_details_1[1], 1, 0, 0, line!());

        let one_b: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", zero_a).unwrap();
        let header_details_2: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "header_details",
            vec![zero_a, one_b.clone()]
        )
        .unwrap();
        let entry_details_2: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "entry_details",
            vec![zero_hash.clone(), one_hash.clone()]
        )
        .unwrap();
        check(&header_details_2[0], 0, 0);
        check(&header_details_2[1], 1, 0);
        check_entry(&entry_details_2[0], 0, 2, 0, line!());
        check_entry(&entry_details_2[1], 1, 0, 0, line!());

        let two: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "inc", one_b).unwrap();
        let header_details_3: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "header_details",
            vec![one_b.clone(), two]
        )
        .unwrap();
        let entry_details_3: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "entry_details",
            vec![zero_hash.clone(), one_hash.clone(), two_hash.clone()]
        )
        .unwrap();
        check(&header_details_3[0], 1, 0);
        check(&header_details_3[1], 2, 0);
        check_entry(&entry_details_3[0], 0, 2, 0, line!());
        check_entry(&entry_details_3[1], 1, 1, 0, line!());
        check_entry(&entry_details_3[2], 2, 0, 0, line!());

        let zero_b: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::Crud, "dec", one_a).unwrap();
        let header_details_4: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "header_details",
            vec![one_a, one_b, zero_b]
        )
        .unwrap();
        let entry_details_4: Vec<Option<Details>> = crate::call_test_ribosome!(
            host_access,
            TestWasm::Crud,
            "entry_details",
            vec![zero_hash, one_hash, two_hash]
        )
        .unwrap();
        check(&header_details_4[0], 1, 1);
        check(&header_details_4[1], 1, 0);
        check_entry(&entry_details_4[0], 0, 2, 0, line!());
        check_entry(&entry_details_4[1], 1, 1, 1, line!());
        check_entry(&entry_details_4[2], 2, 0, 0, line!());

        match header_details_4[2] {
            Some(Details::Element(ref element_details)) => {
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
