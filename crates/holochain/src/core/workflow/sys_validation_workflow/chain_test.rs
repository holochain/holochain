use super::*;
use crate::sweettest::*;
use crate::test_utils::consistency_10s;
use crate::test_utils::inline_zomes::simple_create_read_zome;

/// Unfortunately this test doesn't do anything yet because
/// failing a chain validation is just a log error so the only way to
/// verify this works is to run this with logging and check it outputs
/// use `RUST_LOG=[agent_activity]=warn`
#[tokio::test(flavor = "multi_thread")]
#[ignore = "TODO: complete when chain validation returns actual error"]
async fn sys_validation_agent_activity_test() {
    observability::test_run().ok();

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_create_read_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    let ((cell_1,), (cell_2,)) = apps.into_tuples();

    let a: HeaderHash = conductors[0]
        .call(&cell_1.zome("simple"), "create", ())
        .await;

    let b: HeaderHash = conductors[0]
        .call(&cell_1.zome("simple"), "create", ())
        .await;

    let changed = cell_1
        .dht_db()
        .async_commit(|txn| {
            DatabaseResult::Ok(txn.execute(
                "UPDATE Header SET seq = 4 WHERE hash = ? OR hash = ?",
                [a, b],
            )?)
        })
        .await
        .unwrap();

    assert_eq!(changed, 2);

    conductors.exchange_peer_info().await;
    consistency_10s(&[&cell_1, &cell_2]).await;
}
