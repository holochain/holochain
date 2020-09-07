use crate::{
    conductor::{dna_store::MockDnaStore, ConductorHandle},
    core::ribosome::ZomeCallInvocation,
    core::{
        state::element_buf::ElementBuf,
        workflow::incoming_dht_ops_workflow::IncomingDhtOpsWorkspace,
    },
    test_utils::host_fn_api::*,
    test_utils::setup_app,
};
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use holo_hash::{AnyDhtHash, DhtOpHash, EntryHash, HeaderHash};
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError};
use holochain_state::fresh_reader_test;
use holochain_types::{
    app::InstalledCell, cell::CellId, dht_op::DhtOpLight, dna::DnaDef, dna::DnaFile, fixt::*,
    test_utils::fake_agent_pubkey_1, test_utils::fake_agent_pubkey_2, validate::ValidationStatus,
    Entry,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::HostInput;
use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};
use tracing::*;

#[tokio::test(threaded_scheduler)]
async fn app_validation_workflow_test() {
    observability::test_run().ok();

    let dna_file = DnaFile::new(
        DnaDef {
            name: "app_validation_workflow_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::Validate.into()].into(),
        },
        vec![TestWasm::Validate.into()],
    )
    .await
    .unwrap();

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    let bob_agent_id = fake_agent_pubkey_2();
    let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
    let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

    let mut dna_store = MockDnaStore::new();

    dna_store.expect_get().return_const(Some(dna_file.clone()));
    dna_store.expect_add_dnas::<Vec<_>>().return_const(());
    dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());
    dna_store.expect_get_entry_def().return_const(None);

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(
            "test_app",
            vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        )],
        dna_store,
    )
    .await;

    run_test(alice_cell_id, bob_cell_id, handle.clone(), &dna_file).await;

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

async fn run_test(
    alice_cell_id: CellId,
    bob_cell_id: CellId,
    handle: ConductorHandle,
    dna_file: &DnaFile,
) {
    let invocation = new_invocation(&bob_cell_id, "always_validates", ()).unwrap();
    handle.call_zome(invocation).await.unwrap().unwrap();
    // Some time for ops to reach alice and run through validation
    tokio::time::delay_for(Duration::from_millis(1000)).await;

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        let res: Vec<_> = fresh_reader_test!(alice_env, |r| {
            workspace
                .validation_limbo
                .iter(&r)
                .unwrap()
                .map(|(k, i)| Ok((k.to_vec(), i)))
                .collect()
                .unwrap()
        });
        {
            let s = debug_span!("inspect_ops");
            let _g = s.enter();
            let element_buf = ElementBuf::vault(alice_env.clone().into(), true).unwrap();
            for (k, i) in &res {
                let hash = DhtOpHash::from_raw_bytes(k.clone());
                let el = element_buf.get_element(&i.op.header_hash()).unwrap();
                debug!(?hash, ?i, op_in_val = ?el);
            }
        }
        assert_eq!(
            fresh_reader_test!(alice_env, |r| {
                workspace
                    .validation_limbo
                    .iter(&r)
                    .unwrap()
                    .count()
                    .unwrap()
            }),
            0
        );
        // Integration should have 3 ops in it
        // Plus another 16 for genesis + init
        let res: Vec<_> = fresh_reader_test!(alice_env, |r| {
            workspace
                .integrated_dht_ops
                .iter(&r)
                .unwrap()
                // Every op should be valid
                .inspect(|(_, i)| {
                    // let s = debug_span!("inspect_ops");
                    // let _g = s.enter();
                    debug!(?i.op);
                    assert_eq!(i.validation_status, ValidationStatus::Valid);
                    Ok(())
                })
                .map(|(_, i)| Ok(i))
                .collect()
                .unwrap()
        });
        {
            let s = debug_span!("inspect_ops");
            let _g = s.enter();
            let element_buf = ElementBuf::vault(alice_env.clone().into(), true).unwrap();
            for i in &res {
                let el = element_buf.get_element(&i.op.header_hash()).unwrap();
                debug!(?i.op, op_in_buf = ?el);
            }
        }

        assert_eq!(res.len(), 3 + 16);
    }

    let (invalid_header_hash, invalid_entry_hash) =
        commit_invalid(&bob_cell_id, &handle, dna_file).await;
    let invalid_entry_hash: AnyDhtHash = invalid_entry_hash.into();

    tokio::time::delay_for(Duration::from_millis(1000)).await;

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        assert_eq!(
            fresh_reader_test!(alice_env, |r| {
                workspace
                    .validation_limbo
                    .iter(&r)
                    .unwrap()
                    .count()
                    .unwrap()
            }),
            0
        );
        // Integration should have 3 ops in it
        // StoreEntry should be invalid.
        // RegisterAgentActivity and StoreElement don't run app validation
        // So they will be valid.
        // Plus another 19 from the previous calls
        let res: Vec<_> = fresh_reader_test!(alice_env, |r| {
            workspace
                .integrated_dht_ops
                .iter(&r)
                .unwrap()
                // Every op should be valid
                .inspect(|(_, i)| {
                    match &i.op {
                        DhtOpLight::StoreEntry(hh, _, eh)
                            if eh == &invalid_entry_hash && hh == &invalid_header_hash =>
                        {
                            assert_eq!(i.validation_status, ValidationStatus::Rejected)
                        }
                        // DhtOpLight::StoreElement(hh, _, _) if hh == &invalid_header_hash => {
                        //     assert_eq!(i.validation_status, ValidationStatus::Rejected)
                        // }
                        _ => assert_eq!(i.validation_status, ValidationStatus::Valid),
                    }
                    Ok(())
                })
                .map(|(_, i)| Ok(i))
                .collect()
                .unwrap()
        });

        assert_eq!(res.len(), 3 + 19);
    }
}

fn new_invocation<P>(
    cell_id: &CellId,
    func: &str,
    payload: P,
) -> Result<ZomeCallInvocation, SerializedBytesError>
where
    P: TryInto<SerializedBytes, Error = SerializedBytesError>,
{
    Ok(ZomeCallInvocation {
        cell_id: cell_id.clone(),
        zome_name: TestWasm::Validate.into(),
        cap: CapSecretFixturator::new(Unpredictable).next().unwrap(),
        fn_name: func.to_string(),
        payload: HostInput::new(payload.try_into()?),
        provenance: cell_id.agent_pubkey().clone(),
    })
}

// Need to "hack holochain" because otherwise the invalid
// commit is caught by the call zome workflow
async fn commit_invalid(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (HeaderHash, EntryHash) {
    let entry = ThisWasmEntry::NeverValidates;
    let entry_hash = EntryHash::with_data(&Entry::try_from(entry.clone()).unwrap()).await;
    let (bob_env, call_data) = CallData::create(bob_cell_id, handle, dna_file).await;
    // 4
    let invalid_header_hash = commit_entry(
        &bob_env,
        call_data.clone(),
        entry.clone().try_into().unwrap(),
        INVALID_ID,
    )
    .await;

    // Produce and publish these commits
    let mut triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.produce_dht_ops.trigger();
    (invalid_header_hash, entry_hash)
}
