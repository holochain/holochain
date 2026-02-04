use ::fixt::fixt;
use holo_hash::{
    fixt::{ActionHashFixturator, AgentPubKeyFixturator},
    AgentPubKey, HashableContentExtSync, WarrantHash,
};
use holochain_state::{
    prelude::{insert_warrant, test_authored_db},
    query::from_blob,
};
use holochain_types::{
    fixt::SignatureFixturator,
    prelude::{
        ChainIntegrityWarrant, SignedWarrant, Timestamp, Warrant, WarrantProof, WarrantType,
    },
};
use holochain_zome_types::prelude::ChainOpType;

#[tokio::test(flavor = "multi_thread")]
async fn write_invalid_op_warrant_to_database() {
    let test_db = test_authored_db();
    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author: fixt!(AgentPubKey),
            action: (fixt!(ActionHash), fixt!(Signature)),
            chain_op_type: ChainOpType::StoreRecord,
        }),
        fixt!(AgentPubKey),
        Timestamp::now(),
        fixt!(AgentPubKey),
    );
    let signed_warrant = SignedWarrant::new(warrant.clone(), fixt!(Signature));
    let signed_warrant2 = signed_warrant.clone();
    test_db
        .test_write(|txn| insert_warrant(txn, signed_warrant2))
        .unwrap();

    test_db.test_read(move |txn| {
        txn.query_row(
            "SELECT hash, author, timestamp, warrantee, type, blob FROM Warrant",
            [],
            |row| {
                let hash = row.get_unwrap::<_, WarrantHash>(0);
                let author = row.get_unwrap::<_, AgentPubKey>(1);
                let timestamp = row.get_unwrap::<_, Timestamp>(2);
                let warrantee = row.get_unwrap::<_, AgentPubKey>(3);
                let warrant_type = row.get_unwrap::<_, WarrantType>(4);
                assert_eq!(hash, warrant.to_hash());
                assert_eq!(author, warrant.author);
                assert_eq!(timestamp, warrant.timestamp);
                assert_eq!(warrantee, warrant.warrantee);
                assert_eq!(warrant_type, warrant.get_type());
                let actual_warrant =
                    from_blob::<SignedWarrant>(row.get_unwrap::<_, Vec<u8>>(5)).unwrap();
                assert_eq!(actual_warrant, signed_warrant);
                Ok(())
            },
        )
        .unwrap()
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn write_chain_fork_warrant_to_database() {
    let test_db = test_authored_db();
    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
            chain_author: fixt!(AgentPubKey),
            action_pair: (
                (fixt!(ActionHash), fixt!(Signature)),
                (fixt!(ActionHash), fixt!(Signature)),
            ),
            seq: 5,
        }),
        fixt!(AgentPubKey),
        Timestamp::now(),
        fixt!(AgentPubKey),
    );
    let signed_warrant = SignedWarrant::new(warrant.clone(), fixt!(Signature));
    let signed_warrant2 = signed_warrant.clone();
    test_db
        .test_write(|txn| insert_warrant(txn, signed_warrant2))
        .unwrap();

    test_db.test_read(move |txn| {
        txn.query_row(
            "SELECT hash, author, timestamp, warrantee, type, blob FROM Warrant",
            [],
            |row| {
                let hash = row.get_unwrap::<_, WarrantHash>(0);
                let author = row.get_unwrap::<_, AgentPubKey>(1);
                let timestamp = row.get_unwrap::<_, Timestamp>(2);
                let warrantee = row.get_unwrap::<_, AgentPubKey>(3);
                let warrant_type = row.get_unwrap::<_, WarrantType>(4);
                assert_eq!(hash, warrant.to_hash());
                assert_eq!(author, warrant.author);
                assert_eq!(timestamp, warrant.timestamp);
                assert_eq!(warrantee, warrant.warrantee);
                assert_eq!(warrant_type, warrant.get_type());
                let actual_warrant =
                    from_blob::<SignedWarrant>(row.get_unwrap::<_, Vec<u8>>(5)).unwrap();
                assert_eq!(actual_warrant, signed_warrant);
                Ok(())
            },
        )
        .unwrap()
    });
}
