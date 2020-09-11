use crate::{
    conductor::{dna_store::MockDnaStore, ConductorHandle},
    core::ribosome::ZomeCallInvocation,
    core::state::dht_op_integration::IntegratedDhtOpsValue,
    core::state::validation_db::ValidationLimboValue,
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
use holochain_state::{env::EnvironmentWrite, fresh_reader_test};
use holochain_types::{
    app::InstalledCell, cell::CellId, dht_op::DhtOpLight, dna::DnaDef, dna::DnaFile, fixt::*,
    test_utils::fake_agent_pubkey_1, test_utils::fake_agent_pubkey_2, validate::ValidationStatus,
    Entry,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{element::Element, zome::ZomeName, Header, HostInput};
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
            zomes: vec![TestWasm::Validate.into(), TestWasm::ValidateLink.into()].into(),
        },
        vec![TestWasm::Validate.into(), TestWasm::ValidateLink.into()],
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
    let invocation =
        new_invocation(&bob_cell_id, "always_validates", (), TestWasm::Validate).unwrap();
    handle.call_zome(invocation).await.unwrap().unwrap();
    // Some time for ops to reach alice and run through validation
    tokio::time::delay_for(Duration::from_millis(1000)).await;

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        let val = inspect_val_limbo(&alice_env, &workspace);
        assert_eq!(val.len(), 0);
        // Integration should have 3 ops in it
        // Plus another 16 for genesis + init
        let int = inspect_integrated(&alice_env, &workspace);
        for (_, i, _) in &int {
            assert_eq!(i.validation_status, ValidationStatus::Valid);
        }

        assert_eq!(int.len(), 3 + 16);
    }

    let (invalid_header_hash, invalid_entry_hash) =
        commit_invalid(&bob_cell_id, &handle, dna_file).await;
    let invalid_entry_hash: AnyDhtHash = invalid_entry_hash.into();

    tokio::time::delay_for(Duration::from_millis(1000)).await;

    fn expected_invalid_entry(
        (hash, i, el): &(DhtOpHash, IntegratedDhtOpsValue, Element),
        line: u32,
        invalid_header_hash: &HeaderHash,
        invalid_entry_hash: &AnyDhtHash,
    ) -> bool {
        let s = format!("\nline:{}\n{:?}\n{:?}\n{:?}", line, hash, i, el);
        match &i.op {
            DhtOpLight::StoreEntry(hh, _, eh)
                if eh == invalid_entry_hash && hh == invalid_header_hash =>
            {
                assert_eq!(i.validation_status, ValidationStatus::Rejected, "{}", s)
            }
            DhtOpLight::StoreElement(hh, _, _) if hh == invalid_header_hash => {
                assert_eq!(i.validation_status, ValidationStatus::Rejected, "{}", s);
            }
            _ => return false,
        }
        true
    }

    fn others((hash, i, el): &(DhtOpHash, IntegratedDhtOpsValue, Element), line: u32) {
        let s = format!("\nline:{}\n{:?}\n{:?}\n{:?}", line, hash, i, el);
        match &i.op {
            // Register agent activity will be invalid if the previous header is invalid
            // This is very hard to track in these tests and this op also doesn't
            // go through app validation so it's more productive to skip it
            DhtOpLight::RegisterAgentActivity(_, _) => (),
            _ => assert_eq!(i.validation_status, ValidationStatus::Valid, "{}", s),
        }
    };

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        let val = inspect_val_limbo(&alice_env, &workspace);
        assert_eq!(val.len(), 0);
        // Integration should have 3 ops in it
        // StoreEntry should be invalid.
        // RegisterAgentActivity doesn't run app validation
        // So they will be valid.
        // Plus another 19 from the previous calls
        let int = inspect_integrated(&alice_env, &workspace);
        for v in &int {
            if !expected_invalid_entry(v, line!(), &invalid_header_hash, &invalid_entry_hash) {
                others(v, line!())
            }
        }

        assert_eq!(int.len(), 3 + 19);
    }

    let invocation =
        new_invocation(&bob_cell_id, "add_valid_link", (), TestWasm::ValidateLink).unwrap();
    handle.call_zome(invocation).await.unwrap().unwrap();

    tokio::time::delay_for(Duration::from_millis(1000)).await;

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        let val = inspect_val_limbo(&alice_env, &workspace);
        assert_eq!(val.len(), 0);
        // Integration should have 6 ops in it
        // Plus another 22 from the previous calls
        let int = inspect_integrated(&alice_env, &workspace);
        for v in &int {
            if !expected_invalid_entry(v, line!(), &invalid_header_hash, &invalid_entry_hash) {
                others(v, line!())
            }
        }
        assert_eq!(int.len(), 6 + 22);
    }
    let invocation =
        new_invocation(&bob_cell_id, "add_invalid_link", (), TestWasm::ValidateLink).unwrap();
    let invalid_link_hash: HeaderHash =
        call_zome_directly(&bob_cell_id, &handle, dna_file, invocation)
            .await
            .try_into()
            .unwrap();
    tokio::time::delay_for(Duration::from_millis(1000)).await;

    fn expected_invalid_link(
        (hash, i, el): &(DhtOpHash, IntegratedDhtOpsValue, Element),
        line: u32,
        invalid_link_hash: &HeaderHash,
    ) -> bool {
        let s = format!("\nline:{}\n{:?}\n{:?}\n{:?}", line, hash, i, el);
        match &i.op {
            // Invalid link
            DhtOpLight::RegisterAddLink(hh, _) if hh == invalid_link_hash => {
                assert_eq!(i.validation_status, ValidationStatus::Rejected, "{}", s)
            }
            DhtOpLight::StoreElement(hh, _, _) if hh == invalid_link_hash => {
                assert_eq!(i.validation_status, ValidationStatus::Rejected, "{}", s)
            }
            _ => return false,
        }
        true
    };
    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        let val = inspect_val_limbo(&alice_env, &workspace);
        assert_eq!(val.len(), 0);
        // Integration should have 9 ops in it
        // Plus another 28 from the previous calls
        let int = inspect_integrated(&alice_env, &workspace);
        for v in &int {
            if !expected_invalid_entry(v, line!(), &invalid_header_hash, &invalid_entry_hash)
                && !expected_invalid_link(v, line!(), &invalid_link_hash)
            {
                others(v, line!())
            }
        }
        assert_eq!(int.len(), 9 + 28);
    }

    let invocation = new_invocation(
        &bob_cell_id,
        "remove_valid_link",
        (),
        TestWasm::ValidateLink,
    )
    .unwrap();
    call_zome_directly(&bob_cell_id, &handle, dna_file, invocation).await;
    tokio::time::delay_for(Duration::from_millis(1000)).await;

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        let val = inspect_val_limbo(&alice_env, &workspace);
        assert_eq!(val.len(), 0);
        // Integration should have 9 ops in it
        // Plus another 37 from the previous calls
        let int = inspect_integrated(&alice_env, &workspace);
        for v in &int {
            if !expected_invalid_entry(v, line!(), &invalid_header_hash, &invalid_entry_hash)
                && !expected_invalid_link(v, line!(), &invalid_link_hash)
            {
                others(v, line!())
            }
        }
        assert_eq!(int.len(), 9 + 37);
    }

    let invocation = new_invocation(
        &bob_cell_id,
        "remove_invalid_link",
        (),
        TestWasm::ValidateLink,
    )
    .unwrap();
    let invalid_remove_hash: HeaderHash =
        call_zome_directly(&bob_cell_id, &handle, dna_file, invocation)
            .await
            .try_into()
            .unwrap();

    tokio::time::delay_for(Duration::from_millis(1000)).await;

    fn expected_invalid_remove_link(
        (hash, i, el): &(DhtOpHash, IntegratedDhtOpsValue, Element),
        line: u32,
        invalid_remove_hash: &HeaderHash,
    ) -> bool {
        if let DhtOpLight::RegisterAgentActivity(_, _) = &i.op {
            return false;
        }
        let s = format!("\nline:{}\n{:?}\n{:?}\n{:?}", line, hash, i, el);
        let sb = SerializedBytes::try_from(&MaybeLinkable::NeverLinkable).unwrap();
        let invalid_link_entry_hash = EntryHash::with_data_sync(&Entry::app(sb).unwrap());
        // Link adds with these base / target are invalid
        if let Header::LinkAdd(la) = el.header() {
            if invalid_link_entry_hash == la.base_address
                || invalid_link_entry_hash == la.target_address
            {
                assert_eq!(i.validation_status, ValidationStatus::Rejected, "{}", s);
                return true;
            }
        }
        match &i.op {
            // The store element for the LinkRemove is invalid
            DhtOpLight::StoreElement(hh, _, _) if hh == invalid_remove_hash => {
                assert_eq!(i.validation_status, ValidationStatus::Rejected, "{}", s)
            }
            // The remove link op is also invalid
            DhtOpLight::RegisterRemoveLink(hh, _) if hh == invalid_remove_hash => {
                assert_eq!(i.validation_status, ValidationStatus::Rejected, "{}", s)
            }
            _ => return false,
        }
        true
    };

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        let val = inspect_val_limbo(&alice_env, &workspace);
        assert_eq!(val.len(), 0);
        // Integration should have 12 ops in it
        // Plus another 46 from the previous calls
        let int = inspect_integrated(&alice_env, &workspace);
        for v in &int {
            if !expected_invalid_entry(v, line!(), &invalid_header_hash, &invalid_entry_hash)
                && !expected_invalid_link(v, line!(), &invalid_link_hash)
                && !expected_invalid_remove_link(v, line!(), &invalid_remove_hash)
            {
                others(v, line!())
            }
        }
        assert_eq!(int.len(), 12 + 46);
    }
}

fn new_invocation<P, Z: Into<ZomeName>>(
    cell_id: &CellId,
    func: &str,
    payload: P,
    zome_name: Z,
) -> Result<ZomeCallInvocation, SerializedBytesError>
where
    P: TryInto<SerializedBytes, Error = SerializedBytesError>,
{
    Ok(ZomeCallInvocation {
        cell_id: cell_id.clone(),
        zome_name: zome_name.into(),
        cap: CapSecretFixturator::new(Unpredictable).next().unwrap(),
        fn_name: func.into(),
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
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
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

async fn call_zome_directly(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
    invocation: ZomeCallInvocation,
) -> SerializedBytes {
    let (bob_env, call_data) = CallData::create(bob_cell_id, handle, dna_file).await;
    // 4
    let output = call_zome_direct(&bob_env, call_data.clone(), invocation).await;

    // Produce and publish these commits
    let mut triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.produce_dht_ops.trigger();
    output
}

#[instrument(skip(env, workspace))]
fn inspect_val_limbo(
    env: &EnvironmentWrite,
    workspace: &IncomingDhtOpsWorkspace,
) -> Vec<(DhtOpHash, ValidationLimboValue, Option<Element>)> {
    debug!("start");
    let element_buf = ElementBuf::pending(env.clone().into()).unwrap();
    fresh_reader_test!(env, |r| {
        workspace
            .validation_limbo
            .iter(&r)
            .unwrap()
            .map(|(k, i)| {
                let hash = DhtOpHash::from_raw_bytes(k.to_vec());
                let el = element_buf.get_element(&i.op.header_hash()).unwrap();
                debug!(?hash, ?i, op_in_val = ?el);
                Ok((hash, i, el))
            })
            .collect()
            .unwrap()
    })
}

#[instrument(skip(env, workspace))]
fn inspect_integrated(
    env: &EnvironmentWrite,
    workspace: &IncomingDhtOpsWorkspace,
) -> Vec<(DhtOpHash, IntegratedDhtOpsValue, Element)> {
    debug!("start");
    let element_buf = ElementBuf::vault(env.clone().into(), true).unwrap();
    let element_buf_reject = ElementBuf::rejected(env.clone().into()).unwrap();
    fresh_reader_test!(env, |r| {
        workspace
            .integrated_dht_ops
            .iter(&r)
            .unwrap()
            .map(|(k, i)| {
                let hash = DhtOpHash::from_raw_bytes(k.to_vec());
                let el = element_buf
                    .get_element(&i.op.header_hash())
                    .unwrap()
                    .or_else(|| element_buf_reject.get_element(&i.op.header_hash()).unwrap())
                    .expect("missing element");
                debug!(?hash, ?i, op_in_int = ?el);
                Ok((hash, i, el))
            })
            .collect()
            .unwrap()
    })
}
