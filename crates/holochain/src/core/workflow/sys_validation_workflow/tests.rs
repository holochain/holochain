use crate::sweettest::await_consistency;
use crate::sweettest::SweetConductorBatch;
use crate::sweettest::SweetDnaFile;
use crate::sweettest::SweetInlineZomes;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::wait_for_integration;
use crate::{conductor::ConductorHandle, core::MAX_TAG_SIZE};
use hdk::prelude::LinkTag;
use holo_hash::ActionHash;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain_sqlite::error::DatabaseResult;
use holochain_state::prelude::*;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_wasm_test_utils::TestWasm;
use rusqlite::named_params;
use rusqlite::Transaction;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn sys_validation_workflow_test() {
    holochain_trace::test_run();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let apps = conductors
        .setup_app(&"test_app", [&dna_file])
        .await
        .unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();
    let alice_cell_id = alice.cell_id().clone();
    let bob_cell_id = bob.cell_id().clone();

    conductors.exchange_peer_info().await;

    run_test(alice_cell_id, bob_cell_id, conductors, dna_file).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_produces_warrants() {
    holochain_trace::test_run();

    let string_entry_def = EntryDef::default_from_id("string");

    #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
    struct AppString(String);

    let zome_sans_validation = SweetInlineZomes::new(vec![string_entry_def], 0).function(
        "create_string",
        move |api, s: AppString| {
            let entry = Entry::app(s.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        },
    );
    let zome_avec_validation =
        zome_sans_validation
            .clone()
            .integrity_function("validate", move |_api, op: Op| {
                dbg!(op.action_type(), op.action_seq());
                if op.action_seq() > 3 {
                    Ok(ValidateCallbackResult::Invalid("nope".to_string()))
                } else {
                    Ok(ValidateCallbackResult::Valid)
                }
            });

    let network_seed = "seed".to_string();

    let (dna_sans, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_sans_validation).await;
    let (dna_avec, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_avec_validation).await;

    let mut conductors = SweetConductorBatch::from_standard_config(3).await;
    let (alice,) = conductors[0]
        .setup_app(&"test_app", [&dna_sans])
        .await
        .unwrap()
        .into_tuple();
    let (bob,) = conductors[1]
        .setup_app(&"test_app", [&dna_avec])
        .await
        .unwrap()
        .into_tuple();
    let (carol,) = conductors[2]
        .setup_app(&"test_app", [&dna_avec])
        .await
        .unwrap()
        .into_tuple();

    conductors.exchange_peer_info().await;

    await_consistency(10, [&alice, &bob, &carol]).await.unwrap();

    let _: ActionHash = conductors[0]
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "create_string",
            AppString("entry1".into()),
        )
        .await;

    await_consistency(10, [&bob, &carol]).await.unwrap();
    todo!("check that the warrant is held by carol");
}

async fn run_test(
    alice_cell_id: CellId,
    bob_cell_id: CellId,
    conductors: SweetConductorBatch,
    dna_file: DnaFile,
) {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    bob_links_in_a_legit_way(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // Integration should have 9 ops in it.
    // Plus another 14 for genesis.
    // Init is not run because we aren't calling the zome.
    let expected_count = 9 + 14;

    let alice_dht_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(
        &alice_dht_db,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    let limbo_is_empty = |txn: &Transaction| {
        let not_empty: bool = txn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOP WHERE when_integrated IS NULL)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        !not_empty
    };

    // holochain_state::prelude::dump_tmp(&alice_dht_db);
    // Validation should be empty
    alice_dht_db.read_async(move |txn| -> DatabaseResult<()> {
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        let num_valid_ops: usize = txn
                .query_row("SELECT COUNT(hash) FROM DhtOP WHERE when_integrated IS NOT NULL AND validation_status = :status",
                named_params!{
                    ":status": ValidationStatus::Valid,
                },
                |row| row.get(0))
                .unwrap();
        assert_eq!(num_valid_ops, expected_count);

        Ok(())
    }).await.unwrap();

    let (bad_update_action, bad_update_entry_hash, link_add_hash) =
        bob_makes_a_large_link(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // Integration should have 14 ops in it + the running tally
    let expected_count = 14 + expected_count;

    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(
        &alice_db,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    let bad_update_entry_hash: AnyDhtHash = bad_update_entry_hash.into();
    let num_valid_ops = move |txn: Transaction| -> DatabaseResult<usize> {
        let valid_ops: usize = txn
                .query_row(
                    "
                    SELECT COUNT(hash) FROM DhtOP
                    WHERE
                    when_integrated IS NOT NULL
                    AND
                    (validation_status = :valid
                        OR (validation_status = :rejected
                            AND (
                                (type = :store_entry AND basis_hash = :bad_update_entry_hash AND action_hash = :bad_update_action)
                                OR
                                (type = :store_record AND action_hash = :bad_update_action)
                                OR
                                (type = :add_link AND action_hash = :link_add_hash)
                                OR
                                (type = :update_content AND action_hash = :bad_update_action)
                                OR
                                (type = :update_record AND action_hash = :bad_update_action)
                            )
                        )
                    )
                    ",
                named_params!{
                    ":valid": ValidationStatus::Valid,
                    ":rejected": ValidationStatus::Rejected,
                    ":store_entry": ChainOpType::StoreEntry,
                    ":store_record": ChainOpType::StoreRecord,
                    ":add_link": ChainOpType::RegisterAddLink,
                    ":update_content": ChainOpType::RegisterUpdatedContent,
                    ":update_record": ChainOpType::RegisterUpdatedRecord,
                    ":bad_update_entry_hash": bad_update_entry_hash,
                    ":bad_update_action": bad_update_action,
                    ":link_add_hash": link_add_hash,
                },
                |row| row.get(0))
                .unwrap();

        Ok(valid_ops)
    };

    alice_db
        .read_async(move |txn| -> DatabaseResult<()> {
            // Validation should be empty
            let limbo = show_limbo(&txn);
            assert!(limbo_is_empty(&txn), "{:?}", limbo);

            Ok(())
        })
        .await
        .unwrap();

    let valid_ops = alice_db.read_async(num_valid_ops.clone()).await.unwrap();
    assert_eq!(valid_ops, expected_count);
}

async fn bob_links_in_a_legit_way(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> ActionHash {
    let base = Post("Bananas are good for you".into());
    let target = Post("Potassium is radioactive".into());
    let base_entry_hash = Entry::try_from(base.clone()).unwrap().to_hash();
    let target_entry_hash = Entry::try_from(target.clone()).unwrap().to_hash();
    let link_tag = LinkTag::from(vec![0; 256]);
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    let zome_index = call_data
        .get_entry_type(TestWasm::Create, POST_INDEX)
        .zome_index;
    // 3
    call_data
        .commit_entry(
            base.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 4
    call_data
        .commit_entry(
            target.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 5
    // Link the entries
    let link_add_address = call_data
        .create_link(
            base_entry_hash.clone().into(),
            target_entry_hash.clone().into(),
            zome_index,
            LinkType(0),
            link_tag.clone(),
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers
        .publish_dht_ops
        .trigger(&"bob_links_in_a_legit_way");
    link_add_address
}

async fn bob_makes_a_large_link(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (ActionHash, EntryHash, ActionHash) {
    let base = Post("Small time base".into());
    let target = Post("Spam it big time".into());
    let bad_update = Msg("This is not the msg you were looking for".into());
    let base_entry_hash = Entry::try_from(base.clone()).unwrap().to_hash();
    let target_entry_hash = Entry::try_from(target.clone()).unwrap().to_hash();
    let bad_update_entry_hash = Entry::try_from(bad_update.clone()).unwrap().to_hash();

    let bytes = (0..MAX_TAG_SIZE + 1)
        .map(|_| 0u8)
        .into_iter()
        .collect::<Vec<_>>();
    let link_tag = LinkTag(bytes);

    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    let zome_index = call_data
        .get_entry_type(TestWasm::Create, POST_INDEX)
        .zome_index;

    // 6
    let original_action_address = call_data
        .commit_entry(
            base.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 7
    call_data
        .commit_entry(
            target.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 8
    // Commit a large action
    let link_add_address = call_data
        .create_link(
            base_entry_hash.clone().into(),
            target_entry_hash.clone().into(),
            zome_index,
            LinkType(0),
            link_tag.clone(),
        )
        .await;

    // 9
    // Commit a bad update entry
    let bad_update_action = call_data
        .update_entry(
            bad_update.clone().try_into().unwrap(),
            original_action_address,
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger(&"bob_makes_a_large_link");
    (bad_update_action, bad_update_entry_hash, link_add_address)
}

//////////////////////
//// Test Ideas
//////////////////////
// These are tests that I think might break
// validation but are too hard to write currently

// 1. Delete points to an action that isn't a NewEntryType.
// ## Comments
// I think this will fail RegisterDeleteBy but pass as StoreRecord
// which is wrong.
// ## Scenario
// 1. Commit a Delete Action that points to a valid EntryHash and
// a ActionHash that exists but is not a NewEntryAction (use CreateLink).
// 2. The Create link is integrated and valid.
// ## Expected
// The Delete action should be invalid for all authorities.

fn show_limbo(txn: &Transaction) -> Vec<DhtOpLite> {
    txn.prepare(
        "
        SELECT DhtOp.type, Action.hash, Action.blob, Action.author
        FROM DhtOp
        LEFT JOIN Action ON DhtOp.action_hash = Action.hash
        WHERE
        when_integrated IS NULL
    ",
    )
    .unwrap()
    .query_and_then([], |row| {
        let op_type: DhtOpType = row.get("type")?;
        let hash: ActionHash = row.get("hash")?;
        match op_type {
            DhtOpType::Chain(op_type) => {
                let action: SignedAction = from_blob(row.get("blob")?)?;
                Ok(ChainOpLite::from_type(op_type, hash, &action)?.into())
            }
            DhtOpType::Warrant(_) => {
                let warrant: SignedWarrant = from_blob(row.get("blob")?)?;
                let author: AgentPubKey = row.get("author")?;
                let ((warrant, timestamp), signature) = warrant.into();
                Ok(WarrantOp::new(warrant, author, signature, timestamp).into())
            }
        }
    })
    .unwrap()
    .collect::<StateQueryResult<Vec<DhtOpLite>>>()
    .unwrap()
}
