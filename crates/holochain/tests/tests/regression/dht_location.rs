use holo_hash::{ActionHash, DhtOpHash, DnaHash, OpBasis};
use holochain::sweettest::{
    await_consistency, SweetConductor, SweetConductorBatch, SweetDnaFile, SweetInlineZomes,
};
use holochain::test_utils::inline_zomes::simple_crud_zome;
use holochain_state::prelude::StateQueryResult;
use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq, Hash)]
struct FoundLocationInfo {
    dht_op_hash: DhtOpHash,
    op_basis: OpBasis,
    storage_center_loc: u32,
    op_id: kitsune2_api::OpId,
}

/// Check that the location of the op_basis always matches the storage center location and that the
/// location that will be reported to Kitsune2 is consistent with both.
///
/// Because we can't create data over time in a unit test (at least not without writing directly
/// to the database and risking making mistakes there), this is about as close as we can get to
/// checking that peers who author or sync data will always agree on the location of that data.
///
/// If the locations and ids are consistent, then we can rely on the tests in Kitsune2 for syncing
/// the DHT model.
#[tokio::test(flavor = "multi_thread")]
async fn dht_location_consistency() {
    holochain_trace::test_run();

    // Set up two conductors with rendezvous configuration
    let mut conductors = SweetConductorBatch::standard(2).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();

    conductors.exchange_peer_info().await;

    // Create some op data
    let alice_zome = alice.zome(SweetInlineZomes::COORDINATOR);
    conductors[0]
        .call::<_, ActionHash>(&alice_zome, "create_string", "hi".to_string())
        .await;
    conductors[0]
        .call::<_, ActionHash>(&alice_zome, "create_string", "hello".to_string())
        .await;
    conductors[0]
        .call::<_, ActionHash>(&alice_zome, "create_string", "alright guv'nor".to_string())
        .await;

    await_consistency(&[alice.clone(), bob.clone()])
        .await
        .unwrap();

    let alice_info = get_location_info_from(&conductors[0], alice.dna_hash().clone());
    let bob_info = get_location_info_from(&conductors[1], bob.dna_hash().clone());

    // Demonstrates that the hash/location calculations are consistent between authoring data
    // locally and reconstructing it after syncing over the network.
    assert_eq!(
        alice_info, bob_info,
        "DHT location info should match across conductors"
    );

    // Now we need to check that the locations are internally consistent with themselves.
    for info in &alice_info {
        assert_eq!(info.storage_center_loc, info.op_basis.get_loc());
        assert_eq!(info.op_id.loc(), info.storage_center_loc);
    }
}

fn get_location_info_from(
    conductor: &SweetConductor,
    dna_hash: DnaHash,
) -> HashSet<FoundLocationInfo> {
    let dht = conductor.get_dht_db(&dna_hash).unwrap();
    dht.test_read(|txn| -> StateQueryResult<HashSet<FoundLocationInfo>> {
        let mut stmt = txn.prepare(
            r#"
        SELECT
          hash,
          basis_hash,
          storage_center_loc
        FROM
          DhtOp
        "#,
        )?;

        let mut out = HashSet::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let dht_op_hash = row.get::<_, DhtOpHash>(0)?;
            let op_basis = row.get::<_, OpBasis>(1)?;
            let storage_center_loc = row.get::<_, u32>(2)?;

            let op_id = dht_op_hash.to_located_k2_op_id(&op_basis);

            out.insert(FoundLocationInfo {
                dht_op_hash,
                op_basis,
                storage_center_loc,
                op_id,
            });
        }

        Ok(out)
    })
    .unwrap()
}
