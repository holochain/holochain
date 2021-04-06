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
