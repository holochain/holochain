use crate::test_utils::{test_cell_db, TestEnv};
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_sqlite::db::ReadManager;
use holochain_types::prelude::*;
use holochain_zome_types::test_utils::fake_agent_pubkey_1;
use ChainStatus::*;

fn setup() -> (TestEnv, MetadataBuf, Create, Create, AgentPubKey) {
    observability::test_run().ok();
    let test_db = test_cell_db();
    let meta_buf = MetadataBuf::vault(test_db.env().into()).unwrap();
    let agent_pubkey = fake_agent_pubkey_1();
    let mut h1 = fixt!(Create);
    let mut h2 = fixt!(Create);
    h1.author = agent_pubkey.clone();
    h2.author = agent_pubkey.clone();
    (test_db, meta_buf, h1, h2, agent_pubkey)
}

/// TEST: The following chain status transitions have the proper precedence
#[test]
fn add_chain_status_test() {
    // - Invalid overwrites valid
    let prev_status = Valid(ChainHead {
        action_seq: 1,
        hash: fixt!(ActionHash),
    });
    let incoming_status = Invalid(ChainHead {
        action_seq: 1,
        hash: fixt!(ActionHash),
    });
    assert_eq!(
        add_chain_status(prev_status, incoming_status.clone()),
        Some(incoming_status.clone())
    );

    // - Invalid overwrites any invalids later in the chain.
    let prev_status = Invalid(ChainHead {
        action_seq: 2,
        hash: fixt!(ActionHash),
    });
    assert_eq!(
        add_chain_status(prev_status.clone(), incoming_status.clone()),
        Some(incoming_status.clone())
    );
    // Reverse and expect reverse result
    assert_eq!(
        add_chain_status(incoming_status.clone(), prev_status.clone()),
        None
    );

    // - Invalid overwrites any forks later in the chain.
    let prev_status = Forked(ChainFork {
        fork_seq: 2,
        first_action: fixt!(ActionHash),
        second_action: fixt!(ActionHash),
    });
    assert_eq!(
        add_chain_status(prev_status.clone(), incoming_status.clone()),
        Some(incoming_status.clone())
    );
    // Reverse and expect reverse result
    assert_eq!(
        add_chain_status(incoming_status.clone(), prev_status.clone()),
        None
    );

    // - Forked overwrites any forks later in the chain.
    let prev_status = Forked(ChainFork {
        fork_seq: 2,
        first_action: fixt!(ActionHash),
        second_action: fixt!(ActionHash),
    });
    let incoming_status = Forked(ChainFork {
        fork_seq: 1,
        first_action: fixt!(ActionHash),
        second_action: fixt!(ActionHash),
    });
    assert_eq!(
        add_chain_status(prev_status.clone(), incoming_status.clone()),
        Some(incoming_status.clone())
    );
    // Reverse and expect reverse result
    assert_eq!(
        add_chain_status(incoming_status.clone(), prev_status.clone()),
        None
    );

    // - Forked overwrites any invalid later in the chain.
    let prev_status = Invalid(ChainHead {
        action_seq: 2,
        hash: fixt!(ActionHash),
    });
    let incoming_status = Forked(ChainFork {
        fork_seq: 1,
        first_action: fixt!(ActionHash),
        second_action: fixt!(ActionHash),
    });
    assert_eq!(
        add_chain_status(prev_status.clone(), incoming_status.clone()),
        Some(incoming_status.clone())
    );
    // Reverse and expect reverse result
    assert_eq!(
        add_chain_status(incoming_status.clone(), prev_status.clone()),
        None
    );

    // - Later Valid actions overwrite earlier Valid.
    let prev_status = Valid(ChainHead {
        action_seq: 1,
        hash: fixt!(ActionHash),
    });
    let incoming_status = Valid(ChainHead {
        action_seq: 2,
        hash: fixt!(ActionHash),
    });
    assert_eq!(
        add_chain_status(prev_status, incoming_status.clone()),
        Some(incoming_status)
    );

    // - If there are two Valid status at the same seq num then insert an Fork.
    let hashes: Vec<_> = ActionHashFixturator::new(Predictable).take(2).collect();
    let prev_status = Valid(ChainHead {
        action_seq: 1,
        hash: hashes[0].clone(),
    });
    let incoming_status = Valid(ChainHead {
        action_seq: 1,
        hash: hashes[1].clone(),
    });
    let expected = Forked(ChainFork {
        fork_seq: 1,
        first_action: hashes[0].clone(),
        second_action: hashes[1].clone(),
    });
    assert_eq!(
        add_chain_status(prev_status, incoming_status),
        Some(expected)
    );

    // Empty doesn't overwrite
    let prev_status = Valid(ChainHead {
        action_seq: 1,
        hash: fixt!(ActionHash),
    });
    assert_eq!(add_chain_status(prev_status, ChainStatus::Empty), None);

    // Same doesn't overwrite
    let prev_status = Valid(ChainHead {
        action_seq: 1,
        hash: fixt!(ActionHash),
    });
    assert_eq!(add_chain_status(prev_status.clone(), prev_status), None);
    let prev_status = Forked(ChainFork {
        fork_seq: 2,
        first_action: fixt!(ActionHash),
        second_action: fixt!(ActionHash),
    });
    assert_eq!(add_chain_status(prev_status.clone(), prev_status), None);
    let prev_status = Invalid(ChainHead {
        action_seq: 2,
        hash: fixt!(ActionHash),
    });
    assert_eq!(add_chain_status(prev_status.clone(), prev_status), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn check_different_seq_num_on_separate_queries() {
    let (_te, mut meta_buf, mut h1, mut h2, agent_pubkey) = setup();
    h1.action_seq = 1;
    h2.action_seq = 2;
    meta_buf
        .register_activity(&h1.into(), ValidationStatus::Valid)
        .unwrap();
    meta_buf
        .register_activity(&h2.into(), ValidationStatus::Valid)
        .unwrap();

    let mut conn = meta_buf.env().conn().unwrap();
    conn.with_reader_test(|mut reader| {
        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 1);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            1
        );
        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 2);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            1
        );
        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 0);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            0
        );
        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 3);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            0
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn check_equal_seq_num_on_same_query() {
    let (_te, mut meta_buf, mut h1, mut h2, agent_pubkey) = setup();
    h1.action_seq = 1;
    h2.action_seq = 1;
    let h1: Action = h1.into();
    let h2: Action = h2.into();
    meta_buf
        .register_activity(&h1, ValidationStatus::Valid)
        .unwrap();
    meta_buf
        .register_activity(&h2, ValidationStatus::Valid)
        .unwrap();

    let mut conn = meta_buf.env().conn().unwrap();
    conn.with_reader_test(|mut reader| {
        let k = ChainItemKey::new(&h1, ValidationStatus::Valid);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            1
        );
        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 1);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            2
        );
        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 2);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            0
        );
        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 0);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            0
        );
        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 3);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            0
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn chain_item_keys_ser() {
    let (_te, mut meta_buf, mut h, _, agent_pubkey) = setup();
    h.action_seq = 1;
    let h = Action::Create(h);
    let expect_hash = ActionHash::with_data_sync(&h);
    meta_buf
        .register_activity(&h, ValidationStatus::Valid)
        .unwrap();

    let mut conn = meta_buf.env().conn().unwrap();
    conn.with_reader_test(|mut reader| {
        let k = ChainItemKey::new(&h, ValidationStatus::Valid);
        assert_eq!(
            meta_buf
                .get_activity(&mut reader, k)
                .unwrap()
                .count()
                .unwrap(),
            1
        );

        let k = ChainItemKey::AgentStatusSequence(agent_pubkey.clone(), ValidationStatus::Valid, 1);
        let mut actions: Vec<_> = meta_buf
            .get_activity(&mut reader, k)
            .unwrap()
            .collect()
            .unwrap();
        assert_eq!(actions.len(), 1);
        println!("expect hash {:?}", expect_hash.clone().into_inner());
        assert_eq!(actions.pop().unwrap().action_hash, expect_hash);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn check_large_seq_queries() {
    let (_te, mut meta_buf, mut h1, mut h2, agent_pubkey) = setup();
    h1.action_seq = 256;
    h2.action_seq = 1;
    let h1_hash = ActionHash::with_data_sync(&Action::Create(h1.clone()));
    let h2_hash = ActionHash::with_data_sync(&Action::Create(h2.clone()));

    meta_buf
        .register_activity(&h1.into(), ValidationStatus::Valid)
        .unwrap();
    meta_buf
        .register_activity(&h2.into(), ValidationStatus::Valid)
        .unwrap();

    let mut conn = meta_buf.env().conn().unwrap();
    conn.with_reader_test(|mut reader| {
        let k = ChainItemKey::Agent(agent_pubkey.clone());
        assert_eq!(
            &meta_buf
                .get_activity_sequence(&mut reader, k)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &[(1, h2_hash), (256, h1_hash)]
        );
    });
}
