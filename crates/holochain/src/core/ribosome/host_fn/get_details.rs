use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
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
) -> Result<Vec<Option<Details>>, RuntimeError> {
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
                        Cascade::from_workspace_and_network(
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
                    result.map_err(|cascade_error| {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string()))
                    })
                })
                .collect();
            Ok(results?)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get_details".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_get_details_test() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Crud).await;

        // simple replica of the internal type for the TestWasm::Crud entry
        #[derive(Clone, Copy, Serialize, Deserialize, SerializedBytes, Debug, PartialEq)]
        struct CounTree(u32);

        let check = |details: &Option<Details>, count, delete| match details {
            Some(Details::Record(ref record_details)) => {
                match record_details.record.entry().to_app_option::<CounTree>() {
                    Ok(Some(CounTree(u))) => assert_eq!(u, count),
                    _ => panic!("failed to deserialize {:?}, {}, {}", details, count, delete),
                }
                assert_eq!(record_details.deletes.len(), delete);
            }
            _ => panic!("no record"),
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

        let zero_hash: EntryHash = conductor.call(&alice, "entry_hash", CounTree(0)).await;
        let one_hash: EntryHash = conductor.call(&alice, "entry_hash", CounTree(1)).await;
        let two_hash: EntryHash = conductor.call(&alice, "entry_hash", CounTree(2)).await;

        let zero_a: ActionHash = conductor.call(&alice, "new", ()).await;
        let action_details_0: Vec<Option<Details>> = conductor
            .call(&alice, "action_details", vec![zero_a.clone()])
            .await;
        let entry_details_0: Vec<Option<Details>> = conductor
            .call(&alice, "entry_details", vec![zero_hash.clone()])
            .await;
        check(&action_details_0[0], 0, 0);
        check_entry(&entry_details_0[0], 0, 0, 0, line!());

        let one_a: ActionHash = conductor.call(&alice, "inc", zero_a.clone()).await;
        let action_details_1: Vec<Option<Details>> = conductor
            .call(
                &alice,
                "action_details",
                vec![zero_a.clone(), one_a.clone()],
            )
            .await;
        let entry_details_1: Vec<Option<Details>> = conductor
            .call(
                &alice,
                "entry_details",
                vec![zero_hash.clone(), one_hash.clone()],
            )
            .await;
        check(&action_details_1[0], 0, 0);
        check(&action_details_1[1], 1, 0);
        check_entry(&entry_details_1[0], 0, 1, 0, line!());
        check_entry(&entry_details_1[1], 1, 0, 0, line!());

        let one_b: ActionHash = conductor.call(&alice, "inc", zero_a.clone()).await;
        let action_details_2: Vec<Option<Details>> = conductor
            .call(
                &alice,
                "action_details",
                vec![zero_a.clone(), one_b.clone()],
            )
            .await;
        let entry_details_2: Vec<Option<Details>> = conductor
            .call(
                &alice,
                "entry_details",
                vec![zero_hash.clone(), one_hash.clone()],
            )
            .await;
        check(&action_details_2[0], 0, 0);
        check(&action_details_2[1], 1, 0);
        check_entry(&entry_details_2[0], 0, 2, 0, line!());
        check_entry(&entry_details_2[1], 1, 0, 0, line!());

        let two: ActionHash = conductor.call(&alice, "inc", one_b.clone()).await;
        let action_details_3: Vec<Option<Details>> = conductor
            .call(&alice, "action_details", vec![one_b.clone(), two])
            .await;
        let entry_details_3: Vec<Option<Details>> = conductor
            .call(
                &alice,
                "entry_details",
                vec![zero_hash.clone(), one_hash.clone(), two_hash.clone()],
            )
            .await;
        check(&action_details_3[0], 1, 0);
        check(&action_details_3[1], 2, 0);
        check_entry(&entry_details_3[0], 0, 2, 0, line!());
        check_entry(&entry_details_3[1], 1, 1, 0, line!());
        check_entry(&entry_details_3[2], 2, 0, 0, line!());

        let zero_b: ActionHash = conductor.call(&alice, "dec", one_a.clone()).await;
        let action_details_4: Vec<Option<Details>> = conductor
            .call(&alice, "action_details", vec![one_a, one_b, zero_b])
            .await;
        let entry_details_4: Vec<Option<Details>> = conductor
            .call(&alice, "entry_details", vec![zero_hash, one_hash, two_hash])
            .await;
        check(&action_details_4[0], 1, 1);
        check(&action_details_4[1], 1, 0);
        check_entry(&entry_details_4[0], 0, 2, 0, line!());
        check_entry(&entry_details_4[1], 1, 1, 1, line!());
        check_entry(&entry_details_4[2], 2, 0, 0, line!());

        match action_details_4[2] {
            Some(Details::Record(ref record_details)) => {
                match record_details.record.entry().as_option() {
                    None => {
                        // this is the delete so it should be none
                    }
                    _ => panic!("delete had a record"),
                }
            }
            _ => panic!("no record"),
        }
    }
}
