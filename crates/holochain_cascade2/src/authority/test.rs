use super::*;
use crate::test_utils::*;
use ghost_actor::dependencies::observability;
use holochain_state::prelude::test_cell_env;

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    observability::test_run().ok();
    let env = test_cell_env();

    let td = EntryTestData::new();

    fill_db(&env.env(), td.store_entry_op.clone());

    // TODO: These are probably irrelevant now
    let options = holochain_p2p::event::GetOptions {
        follow_redirects: false,
        all_live_headers_with_metadata: true,
    };

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone()).unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.delete_entry_header_op.clone());

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone()).unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![td.wire_delete.clone()],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.update_content_op.clone());

    let result = handle_get_entry(env.env().into(), td.hash.clone(), options.clone()).unwrap();
    let expected = WireEntryOps {
        creates: vec![td.wire_create.clone()],
        deletes: vec![td.wire_delete.clone()],
        updates: vec![td.wire_update.clone()],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_element() {
    observability::test_run().ok();
    let env = test_cell_env();

    let td = ElementTestData::new();

    fill_db(&env.env(), td.store_element_op.clone());

    let result = handle_get_element(env.env().into(), td.create_hash.clone()).unwrap();
    let expected = WireElementOps {
        header: Some(td.wire_create.clone()),
        deletes: vec![],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.deleted_by_op.clone());

    let result = handle_get_element(env.env().into(), td.create_hash.clone()).unwrap();
    let expected = WireElementOps {
        header: Some(td.wire_create.clone()),
        deletes: vec![td.wire_delete.clone()],
        updates: vec![],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.update_element_op.clone());

    let result = handle_get_element(env.env().into(), td.create_hash.clone()).unwrap();
    let expected = WireElementOps {
        header: Some(td.wire_create.clone()),
        deletes: vec![td.wire_delete.clone()],
        updates: vec![td.wire_update.clone()],
        entry: Some(td.entry.clone()),
    };
    assert_eq!(result, expected);

    fill_db(&env.env(), td.any_store_element_op.clone());

    let result = handle_get_element(env.env().into(), td.any_header_hash.clone()).unwrap();
    let expected = WireElementOps {
        header: Some(td.any_header.clone()),
        deletes: vec![],
        updates: vec![],
        entry: td.any_entry.clone(),
    };
    assert_eq!(result, expected);
}
