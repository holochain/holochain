use crate::prelude::mutations_helpers::insert_valid_integrated_op;

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn link_queries_are_ordered_by_timestamp() {
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = LinkTestData::new();
    insert_valid_integrated_op(&mut txn, td.create_link_op.clone()).unwrap();
    insert_valid_integrated_op(&mut txn, td.later_create_link_op.clone()).unwrap();
    let links = td.tag_query.run(Txn::from(&txn)).unwrap();
    assert_eq!(links, vec![td.link.clone(), td.later_link.clone()]);
    let links = td.details_tag_query.run(Txn::from(&txn)).unwrap();
    assert_eq!(
        links,
        vec![
            (td.create_link_header.clone(), vec![]),
            (td.later_create_link_header.clone(), vec![])
        ]
    );
}
